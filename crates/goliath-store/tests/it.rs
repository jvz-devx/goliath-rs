//! Owns short, real-process protocol and recovery regressions.

#[path = "it/leases.rs"]
mod leases;

use goliath_model::{Fence, Generation, OwnershipCas, PlayerId, TileId};
use goliath_store::{Blob, Client, Command, Operation, RequestId, Response, StoreFault};
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Server {
    process: Child,
    address: SocketAddr,
}

impl Server {
    fn start(path: PathBuf) -> Self {
        Self::start_with(path, &[])
    }
    fn start_with(path: PathBuf, extra: &[&str]) -> Self {
        let mut process = ProcessCommand::new(env!("CARGO_BIN_EXE_goliath-store"))
            .args([
                "--dir",
                path.to_str().expect("UTF-8 test path"),
                "--compact-every",
                "4",
            ])
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn store");
        let mut line = String::new();
        BufReader::new(process.stdout.take().expect("store stdout"))
            .read_line(&mut line)
            .expect("read ready line");
        let address = line
            .trim()
            .strip_prefix("LISTEN ")
            .expect("store ready")
            .parse()
            .expect("parse listener");
        Self { process, address }
    }
    fn client(&self) -> Client {
        Client::new(self.address, Duration::from_secs(2), 2).expect("valid client")
    }
    fn call(&self, id: u128, operation: Operation) -> Response {
        self.client()
            .call(&Command {
                id: RequestId(id),
                operation,
            })
            .expect("store response")
    }
    fn kill(&mut self) {
        self.process.kill().expect("kill store");
        self.process.wait().expect("reap store");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local/runs");
    fs::create_dir_all(&root).expect("create run parent");
    root.join(format!(
        "e5-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn claim(server: &Server, id: u128, tile: usize) -> Fence {
    match server.call(id, Operation::Claim(TileId(tile))) {
        Response::Claimed(fence) => fence,
        response => panic!("claim: {response:?}"),
    }
}

#[test]
fn committed_blobs_receipts_and_epochs_survive_sigkill_and_compaction() {
    let path = path();
    let mut server = Server::start(path.clone());
    let source = claim(&server, 1, 100);
    let target = claim(&server, 2, 900);
    let blob = Blob {
        version: 42,
        bytes: vec![0, 1, 255],
    };
    assert_eq!(
        server.call(
            3,
            Operation::Create {
                player: PlayerId(999),
                owner: source,
                blob: blob.clone()
            }
        ),
        Response::Committed(Generation(0))
    );
    let operation = Operation::Commit {
        player: PlayerId(999),
        cas: OwnershipCas {
            generation: Generation(0),
            source,
            target,
        },
        blob: blob.clone(),
    };
    assert_eq!(
        server.call(4, operation.clone()),
        Response::Committed(Generation(1))
    );
    assert_eq!(server.call(5, Operation::Compact), Response::Ok);
    assert_eq!(fs::metadata(path.join("journal")).unwrap().len(), 0);
    server.kill();
    let mut restarted = Server::start(path.clone());
    assert_eq!(
        restarted.call(4, operation.clone()),
        Response::Committed(Generation(1))
    );
    let Response::Player(Some(record)) = restarted.call(0, Operation::Read(PlayerId(999))) else {
        panic!("missing record")
    };
    assert_eq!(record.inventory, blob);
    assert_eq!(record.owner, target.tile);
    let fresh = claim(&restarted, 6, 100);
    assert!(fresh.epoch.0 > source.epoch.0);
    assert_ne!(fresh.process, source.process);
    assert_eq!(
        restarted.call(4, operation),
        Response::Fault(StoreFault::StaleFence)
    );
    assert_eq!(
        restarted.call(
            7,
            Operation::Heartbeat {
                fence: source,
                counter: 1
            }
        ),
        Response::Fault(StoreFault::StaleFence)
    );
    assert_eq!(restarted.call(8, Operation::Release(fresh)), Response::Ok);
    assert_eq!(restarted.call(8, Operation::Release(fresh)), Response::Ok);
    assert_eq!(
        restarted.call(
            9,
            Operation::Heartbeat {
                fence: fresh,
                counter: 2
            }
        ),
        Response::Fault(StoreFault::StaleFence)
    );
    restarted.kill();
    drop(server);
    drop(restarted);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn request_ids_bind_exact_payloads_and_only_one_writer_can_open_a_store() {
    let path = path();
    let mut server = Server::start(path.clone());
    let source = claim(&server, 1, 0);
    assert_eq!(
        server.call(1, Operation::Claim(TileId(1))),
        Response::Fault(StoreFault::RequestConflict)
    );
    assert!(goliath_store::Engine::open(&path, goliath_store::SyncMode::Full, 0).is_err());
    let blob = Blob {
        version: 1,
        bytes: vec![8; 1024],
    };
    assert_eq!(
        server.call(
            2,
            Operation::Create {
                player: PlayerId(0),
                owner: source,
                blob: blob.clone()
            }
        ),
        Response::Committed(Generation(0))
    );
    let target = claim(&server, 3, 1);
    let original = Operation::Commit {
        player: PlayerId(0),
        cas: OwnershipCas {
            generation: Generation(0),
            source,
            target,
        },
        blob: blob.clone(),
    };
    assert_eq!(
        server.call(4, original.clone()),
        Response::Committed(Generation(1))
    );
    let mut changed = blob;
    changed.bytes.push(9);
    assert_eq!(
        server.call(
            5,
            Operation::Commit {
                player: PlayerId(0),
                cas: OwnershipCas {
                    generation: Generation(0),
                    source,
                    target
                },
                blob: changed
            }
        ),
        Response::Fault(StoreFault::GenerationConflict)
    );
    assert_eq!(
        server.call(
            6,
            Operation::Commit {
                player: PlayerId(0),
                cas: OwnershipCas {
                    generation: Generation(1),
                    source: target,
                    target: source
                },
                blob: Blob {
                    version: 2,
                    bytes: vec![]
                }
            }
        ),
        Response::Committed(Generation(2))
    );
    assert_eq!(server.call(4, original), Response::Committed(Generation(1)));
    let Response::Player(Some(record)) = server.call(0, Operation::Read(PlayerId(0))) else {
        panic!("missing record")
    };
    assert_eq!(record.generation, Generation(2));
    server.kill();
    drop(server);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn dropped_response_retries_once_and_concurrent_cas_has_one_winner() {
    let path = path();
    let mut server = Server::start(path.clone());
    let source = claim(&server, 1, 0);
    let target = claim(&server, 2, 1);
    let blob = Blob {
        version: 1,
        bytes: vec![42],
    };
    assert_eq!(
        server.call(
            3,
            Operation::Create {
                player: PlayerId(0),
                owner: source,
                blob: blob.clone()
            }
        ),
        Response::Committed(Generation(0))
    );
    let command = Command {
        id: RequestId(4),
        operation: Operation::Commit {
            player: PlayerId(0),
            cas: OwnershipCas {
                generation: Generation(0),
                source,
                target,
            },
            blob,
        },
    };
    let mut abandoned = TcpStream::connect(server.address).unwrap();
    goliath_store::write_request(&mut abandoned, &command, Duration::from_secs(1)).unwrap();
    drop(abandoned);
    assert_eq!(
        server.client().call(&command).unwrap(),
        Response::Committed(Generation(1))
    );
    let mut workers = Vec::new();
    for index in 0..8 {
        let client = server.client();
        workers.push(std::thread::spawn(move || {
            client
                .call(&Command {
                    id: RequestId(10 + index),
                    operation: Operation::Commit {
                        player: PlayerId(0),
                        cas: OwnershipCas {
                            generation: Generation(1),
                            source: target,
                            target: source,
                        },
                        blob: Blob {
                            version: 1,
                            bytes: vec![index as u8],
                        },
                    },
                })
                .expect("concurrent response")
        }));
    }
    let responses: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(
        responses
            .iter()
            .filter(|r| **r == Response::Committed(Generation(2)))
            .count(),
        1
    );
    assert_eq!(
        responses
            .iter()
            .filter(|r| **r == Response::Fault(StoreFault::GenerationConflict))
            .count(),
        7
    );
    server.kill();
    drop(server);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn blocking_client_reconnects_after_a_proxy_discards_the_real_store_response() {
    let path = path();
    let mut server = Server::start(path.clone());
    let source = claim(&server, 1, 0);
    let target = claim(&server, 2, 1);
    assert_eq!(
        server.call(
            3,
            Operation::Create {
                player: PlayerId(0),
                owner: source,
                blob: Blob {
                    version: 1,
                    bytes: vec![]
                }
            }
        ),
        Response::Committed(Generation(0))
    );
    let command = Command {
        id: RequestId(4),
        operation: Operation::Commit {
            player: PlayerId(0),
            cas: OwnershipCas {
                generation: Generation(0),
                source,
                target,
            },
            blob: Blob {
                version: 1,
                bytes: vec![5],
            },
        },
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let backend = server.address;
    let proxy = std::thread::spawn(move || {
        let timeout = Duration::from_secs(2);
        let mut first = None;
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().expect("proxy connection");
            if attempt == 0 {
                let warm = goliath_store::read_request(&mut socket, timeout)
                    .expect("warm reused connection");
                assert_eq!(warm.operation, Operation::Directory);
                let response = Client::new(backend, timeout, 1)
                    .unwrap()
                    .call(&warm)
                    .unwrap();
                goliath_store::write_response(&mut socket, &response, timeout).unwrap();
            }
            let incoming =
                goliath_store::read_request(&mut socket, timeout).expect("proxy request");
            if let Some(first) = &first {
                assert_eq!(first, &incoming);
            } else {
                first = Some(incoming.clone());
            }
            let response = Client::new(backend, timeout, 1)
                .expect("backend client")
                .call(&incoming)
                .expect("real backend response");
            if attempt == 1 {
                goliath_store::write_response(&mut socket, &response, timeout)
                    .expect("forward retry response");
            }
        }
    });
    let client = Client::new(address, Duration::from_secs(2), 2).unwrap();
    assert!(matches!(
        client
            .call(&Command {
                id: RequestId(0),
                operation: Operation::Directory
            })
            .unwrap(),
        Response::Directory(_)
    ));
    assert_eq!(
        client.call(&command).unwrap(),
        Response::Committed(Generation(1))
    );
    proxy.join().unwrap();
    let Response::Player(Some(record)) = server.call(0, Operation::Read(PlayerId(0))) else {
        panic!("missing player")
    };
    assert_eq!(record.generation, Generation(1));
    server.kill();
    drop(server);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn client_reuses_one_socket_and_fresh_mode_still_connects_per_call() {
    for fresh in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let timeout = Duration::from_secs(2);
            let (mut stream, _) = listener.accept().unwrap();
            for index in 0..2 {
                if index == 1 && fresh {
                    stream = listener.accept().unwrap().0;
                }
                let command = goliath_store::read_request(&mut stream, timeout).unwrap();
                assert_eq!(command.operation, Operation::Directory);
                goliath_store::write_response(&mut stream, &Response::Directory(vec![]), timeout)
                    .unwrap();
            }
        });
        let client = if fresh {
            Client::fresh(address, Duration::from_secs(2), 1)
        } else {
            Client::new(address, Duration::from_secs(2), 1)
        }
        .unwrap();
        let command = Command {
            id: RequestId(0),
            operation: Operation::Directory,
        };
        let (_, first) = client.call_with_timing(&command).unwrap();
        let (_, second) = client.call_with_timing(&command).unwrap();
        assert!(!first.connect.is_zero());
        assert_eq!(second.connect.is_zero(), !fresh);
        worker.join().unwrap();
    }
}

#[test]
fn existing_wire_frames_can_be_pipelined_on_one_store_connection() {
    let path = path();
    let mut server = Server::start(path.clone());
    let mut stream = TcpStream::connect(server.address).unwrap();
    let timeout = Duration::from_secs(2);
    for id in 1..=2 {
        goliath_store::write_request(
            &mut stream,
            &Command {
                id: RequestId(id),
                operation: Operation::Claim(TileId(id as usize - 1)),
            },
            timeout,
        )
        .unwrap();
    }
    for tile in 0..2 {
        assert!(
            matches!(goliath_store::read_response(&mut stream, timeout).unwrap(), Response::Claimed(fence) if fence.tile == TileId(tile))
        );
    }
    drop(stream);
    server.kill();
    drop(server);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn snapshot_install_before_truncation_and_partial_next_snapshot_recover() {
    let path = path();
    let mut server = Server::start(path.clone());
    let source = claim(&server, 1, 0);
    let target = claim(&server, 2, 1);
    assert_eq!(
        server.call(
            3,
            Operation::Create {
                player: PlayerId(0),
                owner: source,
                blob: Blob {
                    version: 1,
                    bytes: vec![]
                }
            }
        ),
        Response::Committed(Generation(0))
    );
    let old_log = fs::read(path.join("journal")).unwrap();
    assert_eq!(server.call(0, Operation::Compact), Response::Ok);
    server.kill();
    fs::write(path.join("journal"), old_log).unwrap();
    fs::write(path.join("snapshot.next"), b"incomplete future snapshot").unwrap();
    let mut restarted = Server::start(path.clone());
    assert_eq!(
        restarted.call(1, Operation::Claim(TileId(0))),
        Response::Claimed(source)
    );
    assert_eq!(
        restarted.call(
            4,
            Operation::Commit {
                player: PlayerId(0),
                cas: OwnershipCas {
                    generation: Generation(0),
                    source,
                    target
                },
                blob: Blob {
                    version: 2,
                    bytes: vec![7]
                }
            }
        ),
        Response::Committed(Generation(1))
    );
    restarted.kill();
    let mut final_server = Server::start(path.clone());
    let Response::Player(Some(record)) = final_server.call(0, Operation::Read(PlayerId(0))) else {
        panic!("missing player")
    };
    assert_eq!(record.generation, Generation(1));
    assert_eq!(record.inventory.version, 2);
    final_server.kill();
    drop(server);
    drop(restarted);
    drop(final_server);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn interior_corruption_and_corrupt_snapshots_refuse_startup() {
    for snapshot in [false, true] {
        let path = path();
        let mut server = Server::start(path.clone());
        claim(&server, 1, 0);
        claim(&server, 2, 1);
        if snapshot {
            assert_eq!(server.call(0, Operation::Compact), Response::Ok);
        }
        server.kill();
        let file = path.join(if snapshot { "snapshot" } else { "journal" });
        let mut bytes = fs::read(&file).unwrap();
        bytes[0] ^= 1;
        fs::write(&file, bytes).unwrap();
        let output = ProcessCommand::new(env!("CARGO_BIN_EXE_goliath-store"))
            .arg("--dir")
            .arg(&path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains(if snapshot {
                "snapshot checksum/header failure"
            } else {
                "interior journal corruption"
            }),
            "{error}"
        );
        drop(server);
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn concurrent_claims_leave_exactly_one_authorized_process_token() {
    let path = path();
    let mut server = Server::start(path.clone());
    let mut workers = Vec::new();
    for index in 1..=8 {
        let client = server.client();
        workers.push(std::thread::spawn(move || {
            client
                .call(&Command {
                    id: RequestId(index),
                    operation: Operation::Claim(TileId(0)),
                })
                .expect("claim response")
        }));
    }
    let mut authorized = 0;
    for (index, worker) in workers.into_iter().enumerate() {
        let Response::Claimed(fence) = worker.join().unwrap() else {
            panic!("claim failed")
        };
        match server.call(
            100 + index as u128,
            Operation::Heartbeat { fence, counter: 1 },
        ) {
            Response::Ok => authorized += 1,
            Response::Fault(StoreFault::StaleFence) => {}
            response => panic!("heartbeat: {response:?}"),
        }
    }
    assert_eq!(authorized, 1);
    server.kill();
    drop(server);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn uuid_records_and_clock_survive_compaction_without_truncating_uuid() {
    use goliath_store::{ClusterClock, Engine, PlayerKey, SyncMode};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../.local/runs/e10-store-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut engine = Engine::open(&root, SyncMode::Full, 0).unwrap();
    let mut next = 100_000u128;
    let mut execute = |operation| {
        next += 1;
        engine
            .execute(&Command {
                id: RequestId(next),
                operation,
            })
            .unwrap()
    };
    let Response::Claimed(a) = execute(Operation::Claim(TileId(0))) else {
        panic!("claim");
    };
    let Response::Claimed(b) = execute(Operation::Claim(TileId(1))) else {
        panic!("claim");
    };
    let x = PlayerKey((1u128 << 120) | 17);
    let y = PlayerKey((2u128 << 120) | 17);
    let blob = Blob {
        version: 1,
        bytes: vec![7, 9],
    };
    for player in [x, y] {
        assert_eq!(
            execute(Operation::CreateUuid {
                player,
                owner: a,
                blob: blob.clone()
            }),
            Response::Committed(Generation(0))
        );
    }
    assert_eq!(
        execute(Operation::CommitUuid {
            player: x,
            cas: OwnershipCas {
                generation: Generation(0),
                source: a,
                target: b
            },
            blob: blob.clone()
        }),
        Response::Committed(Generation(1))
    );
    assert_eq!(
        execute(Operation::Players { after: None }),
        Response::Players(vec![x, y])
    );
    assert_eq!(
        execute(Operation::Players { after: Some(x) }),
        Response::Players(vec![y])
    );
    let clock = ClusterClock {
        unix_ms: 42,
        game_tick: 5,
        day_tick: 9,
        raining: true,
        thundering: false,
    };
    assert_eq!(
        execute(Operation::SetClock {
            fence: b,
            clock: clock.clone()
        }),
        Response::Fault(StoreFault::StaleFence)
    );
    assert_eq!(
        execute(Operation::SetClock {
            fence: a,
            clock: clock.clone()
        }),
        Response::Ok
    );
    execute(Operation::Compact);
    drop(engine);
    let mut engine = Engine::open(&root, SyncMode::Full, 0).unwrap();
    assert_eq!(
        engine
            .execute(&Command {
                id: RequestId(0),
                operation: Operation::Clock
            })
            .unwrap(),
        Response::Clock(Some(clock))
    );
    for (player, owner, generation) in
        [(x, TileId(1), Generation(1)), (y, TileId(0), Generation(0))]
    {
        let Response::Player(Some(record)) = engine
            .execute(&Command {
                id: RequestId(0),
                operation: Operation::ReadUuid(player),
            })
            .unwrap()
        else {
            panic!("read");
        };
        assert_eq!(
            (record.owner, record.generation, record.inventory),
            (owner, generation, blob.clone())
        );
    }
    drop(engine);
    std::fs::remove_dir_all(root).unwrap();
}
