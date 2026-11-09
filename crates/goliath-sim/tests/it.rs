//! Owns deterministic replay, shrinking and hostile crash-cut tests.

#[test]
fn partitioned_old_owner_stops_locally_and_the_ignored_deadline_control_fails() {
    use goliath_sim::{LeaseMode, lease_partition};
    for lease_ms in [500, 1000, 2000] {
        for seed in 0..1000 {
            let safe = lease_partition(seed, lease_ms, LeaseMode::Enforce);
            assert!(!safe.violated(), "seed {seed} L {lease_ms}: {safe:?}");
            assert!(safe.old_ticks_after_deadline > 0);
            assert!(safe.rejected_actions > 0);
            let control = lease_partition(seed, lease_ms, LeaseMode::IgnoreDeadline);
            assert!(control.violated());
            assert!(control.stale_appends > 0);
            assert!(control.stale_logins > 0);
            assert!(control.overlapping_owners > 0);
            assert!(control.duplicate_identities > 0);
            assert_eq!(safe.takeover_ms, control.takeover_ms);
        }
    }
}

use goliath_model::{ItemId, PlayerId, Policy, TileId};
use goliath_sim::{
    Action, Fault, FaultAt, Mutation, SETUPS, Scenario, Settings, run, run_with_settings, shrink,
};

#[test]
fn every_permanent_protocol_mutation_is_caught() {
    for (mutation, policy) in Mutation::CASES {
        let caught = SETUPS.into_iter().any(|latency| {
            (0..512).any(|seed| {
                let scenario = Scenario::seeded(seed, latency);
                let result = run_with_settings(
                    &scenario,
                    policy,
                    false,
                    Settings {
                        mutation,
                        ..Settings::default()
                    },
                );
                result.violated() || result.losses > 0
            })
        });
        assert!(caught, "{mutation:?} escaped the independent checker");
    }
}

#[test]
fn delayed_abort_exposes_a_stale_cas_without_perfect_failure_detection() {
    let scenario = Scenario::seeded(14, SETUPS[0]);
    let safe = run(&scenario, Policy::P2, false);
    assert!(!safe.violated() && safe.losses == 0);
    let broken = run_with_settings(
        &scenario,
        Policy::P2,
        false,
        Settings {
            mutation: Mutation::UnfencedCas,
            ..Settings::default()
        },
    );
    assert!(broken.stale_writes > 0);
}

#[test]
fn instant_abort_reproduction_preserves_e1_trace_hash() {
    let scenario = Scenario::seeded(6, SETUPS[0]);
    let old = run_with_settings(
        &scenario,
        Policy::P1,
        false,
        Settings {
            abort_delay_ms: 0,
            ..Settings::default()
        },
    );
    assert_eq!(old.hash, 0x4934_31a8_a166_6ee6);
}

#[test]
fn same_seed_has_bit_identical_trace_hash_and_trace() {
    for policy in Policy::ALL {
        let scenario = Scenario::seeded(42, SETUPS[1]);
        let first = run(&scenario, policy, true);
        let second = run(&scenario, policy, true);
        assert_eq!(first.hash, second.hash);
        assert_eq!(first.trace, second.trace);
        assert_ne!(first.hash, 0);
    }
}

#[test]
fn unsafe_controls_fail_on_the_same_workload_p2_p4_p5_survive() {
    let mut failures = [0; 6];
    for seed in 0..48 {
        for (i, policy) in Policy::ALL.into_iter().enumerate() {
            let outcome = run(&Scenario::seeded(seed, SETUPS[0]), policy, false);
            if outcome.violated() {
                failures[i] += 1;
            }
            if matches!(policy, Policy::P2 | Policy::P4 | Policy::P5) {
                assert!(
                    !outcome.violated(),
                    "{policy:?} seed={seed}: {:?}",
                    outcome.first_failure
                );
            }
            assert_eq!(outcome.losses, outcome.loss_recovery + outcome.loss_other);
        }
    }
    assert!(failures[0] > 0 && failures[1] > 0);
    assert!(
        failures[3] > 0,
        "P3's speculative rollback needs immediate global aborts"
    );
}

#[test]
fn shrunk_counterexample_replays_and_is_one_minimal() {
    let scenario = (0..100)
        .map(|seed| Scenario::seeded(seed, SETUPS[0]))
        .find(|s| run(s, Policy::P1, false).violated())
        .unwrap();
    let reduced = shrink(&scenario, Policy::P1);
    assert!(run(&reduced, Policy::P1, false).violated());
    assert_eq!(
        run(&reduced, Policy::P1, true).hash,
        run(&reduced, Policy::P1, true).hash
    );
    for i in 0..reduced.faults.len() {
        let mut candidate = reduced.clone();
        candidate.faults.remove(i);
        assert_eq!(run(&candidate, Policy::P1, false).duplicates, 0);
    }
    for i in 0..reduced.inputs.len() {
        let mut candidate = reduced.clone();
        candidate.inputs.remove(i);
        assert_eq!(run(&candidate, Policy::P1, false).duplicates, 0);
    }
}

#[test]
fn crash_every_millisecond_through_handover_and_checkpoint() {
    for policy in [Policy::P2, Policy::P3, Policy::P4, Policy::P5] {
        for tile in 0..3 {
            for at in 0..180 {
                let scenario = Scenario {
                    seed: 0,
                    latency: SETUPS[1],
                    faults: vec![FaultAt {
                        at,
                        fault: Fault::Crash(TileId(tile)),
                    }],
                    inputs: vec![
                        (0, PlayerId(0), Action::Drop, ItemId(4)),
                        (1, PlayerId(1), Action::PickUp, ItemId(4)),
                        (2, PlayerId(1), Action::Cross(TileId(1)), ItemId(4)),
                        (15, PlayerId(1), Action::Drop, ItemId(4)),
                        (16, PlayerId(2), Action::PickUp, ItemId(4)),
                        (17, PlayerId(2), Action::Store, ItemId(4)),
                        (18, PlayerId(2), Action::Cross(TileId(2)), ItemId(4)),
                    ],
                };
                let outcome = run_with_settings(
                    &scenario,
                    policy,
                    false,
                    Settings {
                        // Retain E1's P3 crash-cut regression, and test its delayed
                        // abort failure independently rather than claiming it safe.
                        abort_delay_ms: if policy == Policy::P3 { 0 } else { 40 },
                        ..Settings::default()
                    },
                );
                assert!(
                    !outcome.violated(),
                    "{policy:?} tile={tile} at={at}: {:?}",
                    outcome.first_failure
                );
            }
        }
    }
}

#[test]
fn p3_speculative_route_duplicates_when_cluster_abort_is_delayed() {
    let scenario = Scenario {
        seed: 0,
        latency: SETUPS[0],
        inputs: vec![
            (0, PlayerId(1), Action::Take, ItemId(0)),
            (6, PlayerId(1), Action::Cross(TileId(1)), ItemId(0)),
        ],
        faults: vec![FaultAt {
            at: 18,
            fault: Fault::Crash(TileId(0)),
        }],
    };
    assert!(run(&scenario, Policy::P3, false).duplicates > 0);
    let old = run_with_settings(
        &scenario,
        Policy::P3,
        false,
        Settings {
            abort_delay_ms: 0,
            ..Settings::default()
        },
    );
    assert!(!old.violated() && old.losses == 0);
}

#[test]
fn incoming_wal_and_concurrent_tick_regression_seeds() {
    for (seed, setting) in [(233, 0), (143, 1), (0, 2)] {
        let outcome = run(&Scenario::seeded(seed, SETUPS[setting]), Policy::P4, false);
        assert!(
            !outcome.violated(),
            "seed={seed}: {:?}",
            outcome.first_failure
        );
    }
}

#[test]
fn durable_policies_do_not_lose_acknowledged_items() {
    for seed in 0..1000 {
        let outcome = run(&Scenario::seeded(seed, SETUPS[2]), Policy::P4, false);
        assert_eq!(outcome.losses, 0, "seed={seed}");
    }
}

#[test]
fn recovery_fails_closed_without_a_store_read() {
    for policy in [Policy::P1, Policy::P2, Policy::P3, Policy::P4] {
        let scenario = Scenario {
            seed: 0,
            latency: SETUPS[0],
            faults: vec![
                FaultAt {
                    at: 0,
                    fault: Fault::StoreDown,
                },
                FaultAt {
                    at: 1,
                    fault: Fault::Crash(TileId(0)),
                },
                FaultAt {
                    at: 100,
                    fault: Fault::Heal,
                },
            ],
            inputs: vec![(10, PlayerId(0), Action::Cross(TileId(1)), ItemId(4))],
        };
        let outcome = run(&scenario, policy, false);
        assert_eq!(outcome.completed, 0);
        assert!(outcome.rejected_inputs > 0);
        assert!(!outcome.violated());
    }
}

#[test]
fn p0_duplicates_with_one_crossing_and_no_faults() {
    let scenario = Scenario {
        seed: 0,
        latency: SETUPS[0],
        faults: vec![],
        inputs: vec![(0, PlayerId(0), Action::Cross(TileId(1)), ItemId(4))],
    };
    assert!(run(&scenario, Policy::P0, false).duplicates > 0);
    assert!(shrink(&scenario, Policy::P0).inputs.len() == 1);
}

#[test]
fn unjournaled_renewals_survive_store_crashes_and_every_planted_bug_is_caught() {
    use goliath_sim::{RenewalFault, RenewalPath, RenewalSetup, Stall, renewal_crash};
    let seeds = 0..150;
    let run = |seed, stall, path, fault| {
        renewal_crash(
            seed,
            RenewalSetup {
                lease_ms: 1000,
                stall,
                path,
                fault,
            },
        )
    };
    let mut barrier_stops = 0;
    let (mut crashes, mut lost) = (0, 0);
    for stall in Stall::ALL {
        for seed in seeds.clone() {
            let journaled = run(seed, stall, RenewalPath::Barrier, RenewalFault::None);
            assert!(
                !journaled.violated(),
                "seed {seed} {stall:?}: {journaled:?}"
            );
            barrier_stops += journaled.stall_stops;
            let early = run(seed, stall, RenewalPath::Unjournaled, RenewalFault::None);
            assert!(!early.violated(), "seed {seed} {stall:?}: {early:?}");
            // A holder that has renewed once never waits behind a barrier again.
            assert_eq!(early.stall_stops, 0, "seed {seed} {stall:?}");
            assert_eq!(early.unjournaled_grants > 0, early.renewals > 0);
            crashes += early.store_crashes;
            lost += early.renewals_lost;
        }
    }
    // The journaled path is what couples a tile's authority to the barrier.
    assert!(barrier_stops > 0);
    // The safe runs really did crash the store and lose granted renewals.
    assert!(
        crashes > 100 && lost > 100,
        "{crashes} crashes, {lost} lost"
    );
    for fault in RenewalFault::PLANTED {
        let caught = seeds
            .clone()
            .filter(|seed| run(*seed, Stall::Severe, RenewalPath::Unjournaled, fault).violated())
            .count();
        assert!(caught > 0, "{} not caught", fault.name());
    }
    let again = run(7, Stall::Tail, RenewalPath::Unjournaled, RenewalFault::None);
    assert_eq!(
        again,
        run(7, Stall::Tail, RenewalPath::Unjournaled, RenewalFault::None)
    );
}
