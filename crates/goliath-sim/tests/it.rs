//! Owns deterministic replay, shrinking and hostile crash-cut tests.

use goliath_model::{ItemId, PlayerId, Policy, TileId};
use goliath_sim::{Action, Fault, FaultAt, SETUPS, Scenario, run, shrink};

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
fn p0_and_p1_fail_on_the_same_workload_safe_policies_survive() {
    let mut failures = [0; 5];
    for seed in 0..48 {
        for (i, policy) in Policy::ALL.into_iter().enumerate() {
            let outcome = run(&Scenario::seeded(seed, SETUPS[0]), policy, false);
            if outcome.violated() {
                failures[i] += 1;
            }
            if i >= 2 {
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
    for policy in [Policy::P2, Policy::P3, Policy::P4] {
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
                let outcome = run(&scenario, policy, false);
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
