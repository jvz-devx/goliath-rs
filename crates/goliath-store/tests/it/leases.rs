//! Owns real-process lease expiry, renewal replay, restart and takeover tests.

use super::{Server, path};
use goliath_model::{Generation, LeaseGrant, LeaseKeeper, OwnershipCas, PlayerId, TileId};
use goliath_store::{
    Blob, Client, ClusterClock, Command, Operation, PlayerKey, RequestId, Response, StoreFault,
};
use std::fs;
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime};

fn leased(server: &Server, id: u128, tile: usize, lease_ms: u64) -> LeaseGrant {
    match server.call(
        id,
        Operation::ClaimLease {
            tile: TileId(tile),
            lease_ms,
        },
    ) {
        Response::Leased(grant) => grant,
        response => panic!("claim lease: {response:?}"),
    }
}

fn wait_until(deadline: Instant) {
    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
}

#[test]
fn expiry_rejects_every_authority_operation_before_retry_receipts() {
    let root = path();
    let mut server = Server::start(root.clone());
    let a = leased(&server, 1, 0, 1000);
    let b = leased(&server, 2, 1, 1000);
    let last_grant = Instant::now();
    let blob = Blob {
        version: 1,
        bytes: vec![7],
    };
    let create = Operation::Create {
        player: PlayerId(1),
        owner: a.fence,
        blob: blob.clone(),
    };
    let create_uuid = Operation::CreateUuid {
        player: PlayerKey(1 << 120),
        owner: a.fence,
        blob: blob.clone(),
    };
    assert_eq!(
        server.call(3, create.clone()),
        Response::Committed(Generation(0))
    );
    assert_eq!(
        server.call(4, create_uuid.clone()),
        Response::Committed(Generation(0))
    );
    let cas = OwnershipCas {
        generation: Generation(0),
        source: a.fence,
        target: b.fence,
    };
    let commit = Operation::Commit {
        player: PlayerId(1),
        cas,
        blob: blob.clone(),
    };
    let commit_uuid = Operation::CommitUuid {
        player: PlayerKey(1 << 120),
        cas,
        blob: blob.clone(),
    };
    assert_eq!(
        server.call(5, commit.clone()),
        Response::Committed(Generation(1))
    );
    assert_eq!(
        server.call(6, commit_uuid.clone()),
        Response::Committed(Generation(1))
    );
    let set_clock = Operation::SetClock {
        fence: a.fence,
        clock: ClusterClock {
            unix_ms: 1,
            game_tick: 2,
            day_tick: 3,
            raining: false,
            thundering: false,
        },
    };
    assert_eq!(server.call(7, set_clock.clone()), Response::Ok);
    assert_eq!(server.call(8, Operation::CheckLease(a.fence)), Response::Ok);
    wait_until(last_grant + Duration::from_millis(1020));
    for (id, operation) in [
        (
            1,
            Operation::ClaimLease {
                tile: TileId(0),
                lease_ms: 1000,
            },
        ),
        (3, create),
        (4, create_uuid),
        (5, commit),
        (6, commit_uuid),
        (7, set_clock),
        (8, Operation::CheckLease(a.fence)),
        (
            9,
            Operation::RenewLease {
                fence: a.fence,
                counter: 1,
            },
        ),
        (10, Operation::Release(a.fence)),
        (11, Operation::BeginTakeover(a.fence)),
        (
            12,
            Operation::Heartbeat {
                fence: a.fence,
                counter: 1,
            },
        ),
        (
            13,
            Operation::Create {
                player: PlayerId(2),
                owner: a.fence,
                blob: blob.clone(),
            },
        ),
        (
            14,
            Operation::CreateUuid {
                player: PlayerKey(2),
                owner: a.fence,
                blob: blob.clone(),
            },
        ),
    ] {
        assert!(matches!(
            server.call(id, operation),
            Response::Fault(StoreFault::LeaseExpired | StoreFault::StaleFence)
        ));
    }
    assert_eq!(
        server.call(15, Operation::Claim(TileId(0))),
        Response::Fault(StoreFault::LeaseRequired)
    );
    let Response::Directory(entries) = server.call(0, Operation::Directory) else {
        panic!("directory")
    };
    assert!(entries.iter().all(|entry| !entry.active));
    let replacement = leased(&server, 16, 0, 1000);
    assert!(replacement.fence.epoch.0 > a.fence.epoch.0);
    assert_eq!(
        server.call(17, Operation::CheckLease(a.fence)),
        Response::Fault(StoreFault::StaleFence)
    );
    server.kill();
    drop(server);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn renewal_replays_and_out_of_order_counters_never_move_the_deadline() {
    let root = path();
    let mut server = Server::start(root.clone());
    let grant = leased(&server, 1, 0, 600);
    let renew = Operation::RenewLease {
        fence: grant.fence,
        counter: 2,
    };
    let sent = Instant::now();
    let response = server.call(2, renew.clone());
    let received = Instant::now();
    assert!(matches!(
        response,
        Response::Leased(LeaseGrant { counter: 2, .. })
    ));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(server.call(2, renew.clone()), response);
    assert_eq!(
        server.call(3, renew),
        Response::Fault(StoreFault::RenewalReplay)
    );
    assert_eq!(
        server.call(
            4,
            Operation::RenewLease {
                fence: grant.fence,
                counter: 1
            }
        ),
        Response::Fault(StoreFault::RenewalReplay)
    );
    assert_eq!(
        server.call(
            5,
            Operation::ClaimLease {
                tile: TileId(0),
                lease_ms: 600
            }
        ),
        Response::Fault(StoreFault::LeaseBusy)
    );
    wait_until(received + Duration::from_millis(620));
    let replacement = leased(&server, 6, 0, 600);
    assert!(sent.elapsed() >= Duration::from_millis(600));
    assert_ne!(replacement.fence, grant.fence);
    assert!(received.elapsed() < Duration::from_millis(750));
    server.kill();
    drop(server);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn delayed_real_renewal_reply_and_wall_clock_steps_cannot_resurrect_keeper() {
    let root = path();
    let mut server = Server::start(root.clone());
    let mut keeper = LeaseKeeper::new(TileId(0), 900, Duration::from_millis(100)).unwrap();
    let ticket = keeper.request(Instant::now()).unwrap();
    let grant = leased(&server, 1, 0, 900);
    assert!(keeper.observe(ticket, grant, Instant::now()));
    let original_deadline = keeper.deadline().unwrap();
    let request = keeper.request(Instant::now()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let backend = server.address;
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let timeout = Duration::from_secs(2);
        let incoming = goliath_store::read_request(&mut stream, timeout).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        let response = Client::new(backend, timeout, 1)
            .unwrap()
            .call(&incoming)
            .unwrap();
        wait_until(original_deadline + Duration::from_millis(20));
        goliath_store::write_response(&mut stream, &response, timeout).unwrap();
    });
    let command = Command {
        id: RequestId(2),
        operation: Operation::RenewLease {
            fence: grant.fence,
            counter: request.counter(),
        },
    };
    let response = Client::new(address, Duration::from_secs(2), 1)
        .unwrap()
        .call(&command)
        .unwrap();
    let Response::Leased(renewed) = response else {
        panic!("real renewal")
    };
    for wall in [
        SystemTime::UNIX_EPOCH,
        SystemTime::now() + Duration::from_secs(86_400),
    ] {
        assert!(wall >= SystemTime::UNIX_EPOCH);
        assert!(!keeper.may_act(Instant::now()));
    }
    assert!(!keeper.observe(request, renewed, Instant::now()));
    assert_eq!(keeper.deadline(), Some(original_deadline));
    assert!(keeper.request(Instant::now()).is_none());
    worker.join().unwrap();
    server.kill();
    drop(server);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restart_mid_lease_waits_full_length_and_preserves_drain_in_log_and_snapshot() {
    for snapshot in [false, true] {
        let root = path();
        let mut server = Server::start(root.clone());
        let grant = leased(&server, 1, 0, 500);
        assert!(matches!(
            server.call(
                2,
                Operation::RenewLease {
                    fence: grant.fence,
                    counter: 1
                }
            ),
            Response::Leased(_)
        ));
        assert_eq!(
            server.call(3, Operation::BeginTakeover(grant.fence)),
            Response::Ok
        );
        if snapshot {
            assert_eq!(server.call(0, Operation::Compact), Response::Ok);
        }
        std::thread::sleep(Duration::from_millis(150));
        server.kill();
        let mut restarted = Server::start(root.clone());
        let reopened = Instant::now();
        assert_eq!(
            restarted.call(
                4,
                Operation::RenewLease {
                    fence: grant.fence,
                    counter: 2
                }
            ),
            Response::Fault(StoreFault::LeaseDraining)
        );
        assert_eq!(
            restarted.call(
                5,
                Operation::ClaimLease {
                    tile: TileId(0),
                    lease_ms: 500
                }
            ),
            Response::Fault(StoreFault::LeaseBusy)
        );
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            restarted.call(
                6,
                Operation::ClaimLease {
                    tile: TileId(0),
                    lease_ms: 500
                }
            ),
            Response::Fault(StoreFault::LeaseBusy)
        );
        wait_until(reopened + Duration::from_millis(520));
        let replacement = leased(&restarted, 7, 0, 500);
        assert_ne!(replacement.fence, grant.fence);
        restarted.kill();
        drop(server);
        drop(restarted);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn forced_takeover_and_release_wait_out_the_last_granted_renewal() {
    for release in [false, true] {
        let root = path();
        let mut server = Server::start(root.clone());
        let grant = leased(&server, 1, 0, 500);
        std::thread::sleep(Duration::from_millis(100));
        let renew_sent = Instant::now();
        assert!(matches!(
            server.call(
                2,
                Operation::RenewLease {
                    fence: grant.fence,
                    counter: 1
                }
            ),
            Response::Leased(_)
        ));
        let renewed = Instant::now();
        assert_eq!(
            server.call(
                3,
                if release {
                    Operation::Release(grant.fence)
                } else {
                    Operation::BeginTakeover(grant.fence)
                }
            ),
            Response::Ok
        );
        assert_eq!(
            server.call(
                4,
                Operation::ClaimLease {
                    tile: TileId(0),
                    lease_ms: 500
                }
            ),
            Response::Fault(StoreFault::LeaseBusy)
        );
        assert_eq!(
            server.call(
                5,
                Operation::RenewLease {
                    fence: grant.fence,
                    counter: 2
                }
            ),
            Response::Fault(if release {
                StoreFault::StaleFence
            } else {
                StoreFault::LeaseDraining
            })
        );
        wait_until(renewed + Duration::from_millis(520));
        let replacement = leased(&server, 6, 0, 500);
        assert!(renew_sent.elapsed() >= Duration::from_millis(500));
        assert_ne!(replacement.fence, grant.fence);
        server.kill();
        drop(server);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn blocking_helpers_claim_and_renew_with_a_single_caller_owned_keeper() {
    let root = path();
    let mut server = Server::start(root.clone());
    let client = server.client();
    let mut keeper = client
        .claim_lease(TileId(0), 1000, Duration::from_millis(100))
        .unwrap();
    assert!(keeper.may_act(Instant::now()));
    assert!(client.renew_lease(&mut keeper).unwrap());
    assert_eq!(
        server.call(1, Operation::BeginTakeover(keeper.fence().unwrap())),
        Response::Ok
    );
    assert!(!client.renew_lease(&mut keeper).unwrap());
    assert!(!keeper.may_act(Instant::now()));
    server.kill();
    drop(server);
    fs::remove_dir_all(root).unwrap();
}
