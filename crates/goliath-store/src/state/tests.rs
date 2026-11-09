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
