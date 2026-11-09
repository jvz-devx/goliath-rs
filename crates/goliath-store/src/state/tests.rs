//! Owns retry-window and post-eviction mutation safety checks.

use super::*;
use std::time::Duration;

fn command(id: u128, operation: Operation) -> Command {
    Command {
        id: RequestId(id),
        operation,
    }
}

fn execute(state: &mut State, command: &Command) -> Response {
    match state.prepare(command) {
        Ok((response, change)) => {
            if let Some(change) = change {
                state.apply(command.clone(), response.clone(), change);
            }
            response
        }
        Err(fault) => Response::Fault(fault),
    }
}

fn claimed(response: Response) -> Fence {
    match response {
        Response::Claimed(fence) => fence,
        Response::Leased(grant) => grant.fence,
        other => panic!("claim failed: {other:?}"),
    }
}

fn fixture(leased: bool) -> (State, Fence, Fence, Instant) {
    let mut state = State::default();
    let now = Instant::now();
    state.set_now(now);
    let mut claim = |id, tile| {
        claimed(execute(
            &mut state,
            &command(
                id,
                if leased {
                    Operation::ClaimLease {
                        tile: TileId(tile),
                        lease_ms: 60_000,
                    }
                } else {
                    Operation::Claim(TileId(tile))
                },
            ),
        ))
    };
    let a = claim(1, 0);
    let b = claim(2, 1);
    (state, a, b, now)
}

fn evict(state: &mut State, original: &Command) {
    for n in 0..CAPACITY {
        // Descending opaque identities must not change acceptance-order eviction.
        let id = 1_000_000 + (CAPACITY - n) as u128;
        assert!(matches!(
            execute(state, &command(id, Operation::Claim(TileId(2)))),
            Response::Claimed(_)
        ));
    }
    assert!(state.receipts.get(&original.id).is_none());
    assert_eq!(state.receipts.len(), CAPACITY);
}

fn blob(value: u8) -> Blob {
    Blob {
        version: 1,
        bytes: vec![value],
    }
}

fn clock(anchor: u64) -> ClusterClock {
    ClusterClock {
        unix_ms: anchor,
        game_tick: 5,
        day_tick: 9,
        raining: false,
        thundering: false,
    }
}

#[test]
fn e24_retry_after_27_seconds_and_peak_intervening_mutations_keeps_original_receipt() {
    let (mut state, _, _, now) = fixture(true);
    let original = command(
        u128::MAX,
        Operation::ClaimLease {
            tile: TileId(3),
            lease_ms: 60_000,
        },
    );
    let accepted = execute(&mut state, &original);
    let next_process = state.next_process;
    // Whole execution-interval overlap bounds the barrier-bearing batches.
    // The overloaded peak61/s plus20/s cadence headroom gives81mutations/s.
    let maximum_intervening = 27 * 81;
    for n in 0..maximum_intervening {
        execute(
            &mut state,
            &command(10_000 + n as u128, Operation::Claim(TileId(2))),
        );
    }
    state.set_now(now + Duration::from_secs(27));
    let current_process = state.next_process;
    assert_eq!(execute(&mut state, &original), accepted);
    assert_eq!(state.next_process, current_process);
    assert_eq!(current_process - next_process, maximum_intervening as u64);
    assert!(CAPACITY / state.receipts.len() >= 4);
}

#[test]
fn e24_snapshot_restores_fifo_order_with_shuffled_request_ids() {
    let (mut state, _, _, _) = fixture(false);
    let original = command(u128::MAX, Operation::Claim(TileId(3)));
    let accepted = execute(&mut state, &original);
    // Evict both fixture claims but keep exactly CAPACITY-1 newer mutations.
    for n in 0..CAPACITY - 1 {
        execute(
            &mut state,
            &command(1_000_000 - n as u128, Operation::Claim(TileId(2))),
        );
    }
    let mut restored = State::restore(&state.snapshot().unwrap()).unwrap();
    assert_eq!(execute(&mut restored, &original), accepted);
    let next = command(2_000_000, Operation::Claim(TileId(2)));
    execute(&mut restored, &next);
    assert!(restored.receipts.get(&original.id).is_none());
    assert_eq!(restored.receipts.len(), CAPACITY);
}

#[test]
fn e24_oversized_legacy_snapshot_is_refused_instead_of_guessing_fifo_age() {
    let mut encoded = Encoder::default();
    encoded.u64(0);
    encoded.count(0).unwrap(); // tiles
    encoded.count(0).unwrap(); // players
    encoded.count(CAPACITY + 1).unwrap();
    for n in 0..CAPACITY + 1 {
        let receipt = command((n + 1) as u128, Operation::Claim(TileId(0)));
        encoded
            .bytes(&crate::codec::encode_command(&receipt).unwrap())
            .unwrap();
        encoded
            .bytes(&crate::codec::encode_response(&Response::Ok).unwrap())
            .unwrap();
    }
    assert!(
        State::restore(&encoded.finish())
            .err()
            .unwrap()
            .to_string()
            .contains("explicit migration")
    );
}

#[test]
fn e24_evicted_creates_and_commits_cannot_duplicate_or_roll_back_player_records() {
    for uuid in [false, true] {
        let (mut state, a, b, _) = fixture(false);
        let player = PlayerId(7);
        let key = PlayerKey(7);
        let create = command(
            100,
            if uuid {
                Operation::CreateUuid {
                    player: key,
                    owner: a,
                    blob: blob(1),
                }
            } else {
                Operation::Create {
                    player,
                    owner: a,
                    blob: blob(1),
                }
            },
        );
        assert_eq!(
            execute(&mut state, &create),
            Response::Committed(Generation(0))
        );
        let cas = OwnershipCas {
            generation: Generation(0),
            source: a,
            target: b,
        };
        let commit = command(
            101,
            if uuid {
                Operation::CommitUuid {
                    player: key,
                    cas,
                    blob: blob(2),
                }
            } else {
                Operation::Commit {
                    player,
                    cas,
                    blob: blob(2),
                }
            },
        );
        assert_eq!(
            execute(&mut state, &commit),
            Response::Committed(Generation(1))
        );
        evict(&mut state, &commit);
        assert_eq!(
            execute(&mut state, &create),
            Response::Fault(StoreFault::AlreadyExists)
        );
        // The domain CAS recognizes its exact already-committed outcome too.
        assert_eq!(
            execute(&mut state, &commit),
            Response::Committed(Generation(1))
        );
        let record = if uuid {
            state.uuid_players.get(&key)
        } else {
            state.players.get(&player)
        }
        .unwrap();
        assert_eq!(record.generation, Generation(1));
        assert_eq!(record.owner, b.tile);
        assert_eq!(record.inventory, blob(2));
        let onward = OwnershipCas {
            generation: Generation(1),
            source: b,
            target: a,
        };
        let next = command(
            102,
            if uuid {
                Operation::CommitUuid {
                    player: key,
                    cas: onward,
                    blob: blob(3),
                }
            } else {
                Operation::Commit {
                    player,
                    cas: onward,
                    blob: blob(3),
                }
            },
        );
        assert_eq!(
            execute(&mut state, &next),
            Response::Committed(Generation(2))
        );
        evict(&mut state, &commit);
        assert_eq!(
            execute(&mut state, &commit),
            Response::Fault(StoreFault::GenerationConflict)
        );
    }
}

#[test]
fn e24_evicted_legacy_heartbeat_cannot_lower_the_counter() {
    let (mut state, a, _, _) = fixture(false);
    let original = command(
        100,
        Operation::Heartbeat {
            fence: a,
            counter: 17,
        },
    );
    assert_eq!(execute(&mut state, &original), Response::Ok);
    execute(
        &mut state,
        &command(
            101,
            Operation::Heartbeat {
                fence: a,
                counter: 20,
            },
        ),
    );
    evict(&mut state, &original);
    assert_eq!(execute(&mut state, &original), Response::Ok);
    assert_eq!(state.tiles[&a.tile].heartbeat, 20);
    assert_eq!(state.current(a.tile), Some(a));
}

#[test]
fn e24_evicted_release_is_idempotent_or_refused_for_both_authority_modes() {
    for leased in [false, true] {
        let (mut state, a, _, _) = fixture(leased);
        let original = command(100, Operation::Release(a));
        assert_eq!(execute(&mut state, &original), Response::Ok);
        evict(&mut state, &original);
        let reply = execute(&mut state, &original);
        assert_eq!(
            reply,
            if leased {
                Response::Fault(StoreFault::StaleFence)
            } else {
                Response::Ok
            }
        );
        assert!(!state.tiles[&a.tile].active);
        assert_eq!(state.current(a.tile), None);
    }
}

#[test]
fn e24_evicted_renewal_is_refused_without_extending_its_deadline() {
    let (mut state, a, _, _) = fixture(true);
    let original = command(
        100,
        Operation::RenewLease {
            fence: a,
            counter: 1,
        },
    );
    assert!(matches!(
        execute(&mut state, &original),
        Response::Leased(_)
    ));
    let deadline = state.leases[&a.tile].deadline();
    evict(&mut state, &original);
    assert_eq!(
        execute(&mut state, &original),
        Response::Fault(StoreFault::RenewalReplay)
    );
    assert_eq!(state.leases[&a.tile].deadline(), deadline);
}

#[test]
fn e24_evicted_takeover_is_idempotent_and_never_shortens_the_grant() {
    let (mut state, a, _, _) = fixture(true);
    let original = command(100, Operation::BeginTakeover(a));
    assert_eq!(execute(&mut state, &original), Response::Ok);
    let deadline = state.leases[&a.tile].deadline();
    evict(&mut state, &original);
    assert_eq!(execute(&mut state, &original), Response::Ok);
    assert!(state.leases[&a.tile].draining());
    assert_eq!(state.leases[&a.tile].deadline(), deadline);
}

#[test]
fn e24_evicted_claim_can_change_the_fence_but_never_creates_two_current_owners() {
    for leased in [false, true] {
        let (mut state, _, _, now) = fixture(leased);
        let operation = if leased {
            Operation::ClaimLease {
                tile: TileId(3),
                lease_ms: 60_000,
            }
        } else {
            Operation::Claim(TileId(3))
        };
        let original = command(100, operation);
        let old = claimed(execute(&mut state, &original));
        evict(&mut state, &original);
        if leased {
            assert_eq!(
                execute(&mut state, &original),
                Response::Fault(StoreFault::LeaseBusy)
            );
            state.set_now(now + Duration::from_secs(61));
        }
        let fresh = claimed(execute(&mut state, &original));
        assert_ne!(fresh, old);
        assert_eq!(state.current(old.tile), Some(fresh));
        assert_eq!(state.check(old), Err(StoreFault::StaleFence));
        assert_eq!(
            state
                .tiles
                .values()
                .filter(|entry| entry.fence.tile == old.tile && entry.active)
                .count(),
            1
        );
    }
}

#[test]
fn e24_evicted_clock_cannot_overwrite_a_newer_anchor() {
    let (mut state, a, _, _) = fixture(false);
    let original = command(
        100,
        Operation::SetClock {
            fence: a,
            clock: clock(10),
        },
    );
    assert_eq!(execute(&mut state, &original), Response::Ok);
    execute(
        &mut state,
        &command(
            101,
            Operation::SetClock {
                fence: a,
                clock: clock(20),
            },
        ),
    );
    evict(&mut state, &original);
    assert_eq!(
        execute(&mut state, &original),
        Response::Fault(StoreFault::RequestConflict)
    );
    assert_eq!(state.clock, Some(clock(20)));
    let identical = command(
        102,
        Operation::SetClock {
            fence: a,
            clock: clock(20),
        },
    );
    assert_eq!(execute(&mut state, &identical), Response::Ok);
    let changed = ClusterClock {
        raining: true,
        ..clock(20)
    };
    assert_eq!(
        execute(
            &mut state,
            &command(
                103,
                Operation::SetClock {
                    fence: a,
                    clock: changed
                }
            )
        ),
        Response::Fault(StoreFault::RequestConflict)
    );
}

#[test]
fn e24_backward_wall_anchor_is_refused_without_touching_monotonic_authority() {
    let (mut state, a, _, now) = fixture(true);
    let deadline = state.leases[&a.tile].deadline();
    execute(
        &mut state,
        &command(
            100,
            Operation::SetClock {
                fence: a,
                clock: clock(20),
            },
        ),
    );
    state.set_now(now + Duration::from_secs(1));
    assert_eq!(
        execute(
            &mut state,
            &command(
                101,
                Operation::SetClock {
                    fence: a,
                    clock: clock(19)
                }
            )
        ),
        Response::Fault(StoreFault::RequestConflict)
    );
    assert_eq!(state.check(a), Ok(()));
    assert_eq!(state.leases[&a.tile].deadline(), deadline);
    assert_eq!(
        execute(
            &mut state,
            &command(
                102,
                Operation::SetClock {
                    fence: a,
                    clock: clock(21)
                }
            )
        ),
        Response::Ok
    );
    assert_eq!(state.leases[&a.tile].deadline(), deadline);
}

#[test]
fn e24_historical_clock_writes_replay_before_the_live_guard_is_enabled() {
    let mut state = State::default();
    let a = claimed(execute(
        &mut state,
        &command(1, Operation::Claim(TileId(0))),
    ));
    assert_eq!(
        execute(
            &mut state,
            &command(
                2,
                Operation::SetClock {
                    fence: a,
                    clock: clock(20)
                }
            )
        ),
        Response::Ok
    );
    assert_eq!(
        execute(
            &mut state,
            &command(
                3,
                Operation::SetClock {
                    fence: a,
                    clock: clock(10)
                }
            )
        ),
        Response::Ok
    );
    assert_eq!(state.clock, Some(clock(10)));
    state.set_now(Instant::now());
    assert_eq!(
        execute(
            &mut state,
            &command(
                4,
                Operation::SetClock {
                    fence: a,
                    clock: clock(9)
                }
            )
        ),
        Response::Fault(StoreFault::RequestConflict)
    );
}

fn renewal(id: u128, fence: Fence, counter: u64) -> Command {
    command(id, Operation::RenewLease { fence, counter })
}

fn leased_tile(lease_ms: u64) -> (State, Fence, Instant) {
    let mut state = State::default();
    let now = Instant::now();
    state.set_now(now);
    let claim = command(
        1,
        Operation::ClaimLease {
            tile: TileId(0),
            lease_ms,
        },
    );
    let fence = claimed(execute(&mut state, &claim));
    (state, fence, now)
}

#[test]
fn e29_unjournaled_renewal_extends_the_lease_a_takeover_then_waits_for() {
    let (mut state, fence, now) = leased_tile(1000);
    let at = |ms| now + Duration::from_millis(ms);
    // The claim is applied but its barrier has not finished.
    assert_eq!(state.renew_unjournaled(&renewal(2, fence, 1), at(10)), None);
    assert_eq!(state.leases[&fence.tile].deadline(), at(1000));
    state.settle();
    let receipts = state.receipts.len();
    let Some(Response::Leased(grant)) = state.renew_unjournaled(&renewal(2, fence, 1), at(300))
    else {
        panic!("settled renewal not answered")
    };
    assert_eq!(
        (grant.fence, grant.counter, grant.lease_ms),
        (fence, 1, 1000)
    );
    assert_eq!(state.leases[&fence.tile].deadline(), at(1300));
    assert_eq!(state.tiles[&fence.tile].heartbeat, 1);
    // Nothing entered the journal's retry window.
    assert_eq!(state.receipts.len(), receipts);
    // The journaled takeover reads the lease the renewal wrote.
    state.set_now(at(400));
    assert_eq!(
        execute(&mut state, &command(3, Operation::BeginTakeover(fence))),
        Response::Ok
    );
    assert_eq!(state.leases[&fence.tile].deadline(), at(1300));
    assert_eq!(
        state.renew_unjournaled(&renewal(4, fence, 2), at(500)),
        None
    );
    state.set_now(at(500));
    assert_eq!(
        execute(&mut state, &renewal(4, fence, 2)),
        Response::Fault(StoreFault::LeaseDraining)
    );
    let claim = command(
        5,
        Operation::ClaimLease {
            tile: TileId(0),
            lease_ms: 1000,
        },
    );
    // The original claim would have expired at 1,000 ms.
    state.set_now(at(1299));
    assert_eq!(
        execute(&mut state, &claim),
        Response::Fault(StoreFault::LeaseBusy)
    );
    state.set_now(at(1300));
    let successor = claimed(execute(&mut state, &claim));
    assert_ne!(successor, fence);
    // The successor is unsettled again, and the old fence is answered nowhere.
    assert_eq!(
        state.renew_unjournaled(&renewal(6, successor, 1), at(1310)),
        None
    );
    assert_eq!(
        state.renew_unjournaled(&renewal(7, fence, 2), at(1310)),
        None
    );
}

#[test]
fn e29_every_refusal_and_remembered_retry_takes_the_journaled_path() {
    let (mut state, fence, now) = leased_tile(1000);
    let at = |ms| now + Duration::from_millis(ms);
    state.settle();
    let other = Fence {
        tile: TileId(9),
        ..fence
    };
    let stale = Fence {
        epoch: goliath_model::Epoch(fence.epoch.0 + 1),
        ..fence
    };
    for (command, when) in [
        (command(2, Operation::CheckLease(fence)), 10),
        (renewal(0, fence, 1), 10),
        (renewal(2, fence, 0), 10),
        (renewal(2, other, 1), 10),
        (renewal(2, stale, 1), 10),
        // The journal already holds a receipt under this identity.
        (renewal(1, fence, 1), 10),
    ] {
        assert_eq!(state.renew_unjournaled(&command, at(when)), None);
    }
    assert_eq!(state.leases[&fence.tile].deadline(), at(1000));
    let first = renewal(2, fence, 1);
    let granted = state.renew_unjournaled(&first, at(100));
    assert!(matches!(granted, Some(Response::Leased(_))));
    // A transport retry gets the same grant and no second extension.
    assert_eq!(state.renew_unjournaled(&first, at(200)), granted);
    assert_eq!(state.leases[&fence.tile].deadline(), at(1100));
    assert_eq!(
        state.renew_unjournaled(&renewal(2, fence, 5), at(200)),
        Some(Response::Fault(StoreFault::RequestConflict))
    );
    // A replayed counter under a fresh identity is the journaled path's refusal.
    assert_eq!(
        state.renew_unjournaled(&renewal(3, fence, 1), at(200)),
        None
    );
    state.set_now(at(200));
    assert_eq!(
        execute(&mut state, &renewal(3, fence, 1)),
        Response::Fault(StoreFault::RenewalReplay)
    );
    // Expiry is exact: the last instant is refused and not revived.
    assert_eq!(
        state.renew_unjournaled(&renewal(4, fence, 2), at(1100)),
        None
    );
    assert_eq!(state.renew_unjournaled(&first, at(1100)), None);
    assert_eq!(state.leases[&fence.tile].deadline(), at(1100));
    // After a journal failure nothing is answered early.
    let (mut state, fence, now) = leased_tile(1000);
    state.settle();
    state.close();
    assert_eq!(
        state.renew_unjournaled(&renewal(2, fence, 1), now + Duration::from_millis(1)),
        None
    );
}

#[test]
fn e29_a_snapshot_keeps_the_counter_and_recovery_waits_a_full_lease() {
    let (mut state, fence, now) = leased_tile(1000);
    let at = |ms| now + Duration::from_millis(ms);
    state.settle();
    for counter in 1..=3 {
        let command = renewal(10 + u128::from(counter), fence, counter);
        assert!(
            state
                .renew_unjournaled(&command, at(counter * 100))
                .is_some()
        );
    }
    // Journal-only recovery: the claim replays, the renewals are gone.
    let mut replayed = State::default();
    let claim = command(
        1,
        Operation::ClaimLease {
            tile: TileId(0),
            lease_ms: 1000,
        },
    );
    assert_eq!(claimed(execute(&mut replayed, &claim)), fence);
    // Snapshot recovery keeps the counter, and neither keeps the retry memo.
    let mut restored = State::restore(&state.snapshot().unwrap()).unwrap();
    assert_eq!(restored.leases[&fence.tile].counter(), 3);
    assert_eq!(replayed.leases[&fence.tile].counter(), 0);
    for recovered in [&mut replayed, &mut restored] {
        recovered.restart_leases().unwrap();
        let restart = recovered.now.unwrap();
        assert!(recovered.leases[&fence.tile].settled());
        assert!(recovered.unjournaled.is_empty());
        // A real restart happens after the last grant, so a full lease from
        // here outlasts that grant's own window.
        assert_eq!(
            recovered.leases[&fence.tile].deadline(),
            restart + Duration::from_millis(1000)
        );
        recovered.set_now(restart + Duration::from_millis(999));
        assert_eq!(
            execute(recovered, &command(50, claim.operation.clone())),
            Response::Fault(StoreFault::LeaseBusy)
        );
        // The holder's next counter is accepted on either recovery.
        assert!(matches!(
            recovered.renew_unjournaled(&renewal(60, fence, 4), restart + Duration::from_millis(5)),
            Some(Response::Leased(_))
        ));
    }
}
