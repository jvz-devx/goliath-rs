//! Owns durable-before-visible operation ordering and fail-closed IO handling.

use crate::codec::{decode_command, encode_command, invalid};
use crate::journal::{Compaction, CompactionReady, Journal};
use crate::state::State;
use crate::{Command, Operation, Response, SyncMode};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Engine {
    journal: Journal,
    state: State,
    sequence: u64,
    since_compaction: u64,
    compact_every: u64,
    failed: bool,
    compaction_in_flight: bool,
    #[cfg(feature = "fault-injection")]
    early_ack: bool,
}

impl Engine {
    /// Duration of the last batch's journal barrier, excluding encoding and append.
    pub fn last_barrier_duration(&self) -> Duration {
        self.journal.last_barrier
    }

    pub fn open(directory: &Path, mode: SyncMode, compact_every: u64) -> io::Result<Self> {
        let (journal, recovery) = Journal::open(directory, mode)?;
        let (mut state, mut sequence) = match recovery.snapshot {
            Some((sequence, bytes)) => (State::restore(&bytes)?, sequence),
            None => (State::default(), 0),
        };
        let mut previous = None;
        let mut since_compaction = 0;
        for (record_sequence, bytes) in recovery.records {
            if previous.is_some_and(|p: u64| p.checked_add(1) != Some(record_sequence))
                || record_sequence == 0
            {
                return Err(invalid("nonconsecutive journal sequence"));
            }
            previous = Some(record_sequence);
            if record_sequence <= sequence {
                continue;
            }
            if sequence.checked_add(1) != Some(record_sequence) {
                return Err(invalid("journal gap after snapshot"));
            }
            let command = decode_command(&bytes)?;
            let (response, change) = state
                .prepare(&command)
                .map_err(|_| invalid("journal violates ownership rules"))?;
            let change = change.ok_or_else(|| invalid("journal contains a nonmutation"))?;
            state.apply(command, response, change);
            sequence = record_sequence;
            since_compaction += 1;
        }
        let mut engine = Self {
            journal,
            state,
            sequence,
            since_compaction,
            compact_every,
            failed: false,
            compaction_in_flight: false,
            #[cfg(feature = "fault-injection")]
            early_ack: false,
        };
        // Finish an interrupted rotation from recovered state, before serving.
        // Never consolidate overlapping log files by copying their bytes.
        if engine.journal.has_rotation()? {
            engine
                .journal
                .compact(engine.sequence, &engine.state.snapshot()?)?;
            engine.since_compaction = 0;
        }
        // Historical commands replay without today's deadlines. Only after all
        // recovery IO completes does every durable lease get a full-L quarantine.
        engine.state.restart_leases()?;
        Ok(engine)
    }

    #[cfg(feature = "fault-injection")]
    pub fn disable_fence_for_test(&mut self) {
        self.state.disable_fence = true;
    }

    #[cfg(feature = "fault-injection")]
    pub fn acknowledge_before_barrier_for_test(&mut self) {
        self.early_ack = true;
        self.power_loss_file_layer_for_test();
    }

    #[cfg(feature = "fault-injection")]
    pub fn power_loss_file_layer_for_test(&mut self) {
        self.journal.volatile_file_cache = Some(Vec::new());
    }

    pub fn execute(&mut self, command: &Command) -> io::Result<Response> {
        let response = self
            .execute_batch(std::slice::from_ref(command))?
            .pop()
            .ok_or_else(|| invalid("missing batch response"))?;
        self.maintain()?;
        Ok(response)
    }

    /// The caller owns queueing and workers. No response, including a read or
    /// retry receipt, escapes the batch until its one journal barrier succeeds.
    pub fn execute_batch(&mut self, commands: &[Command]) -> io::Result<Vec<Response>> {
        if self.failed {
            return Err(io::Error::other(
                "store failed after IO error; restart required",
            ));
        }
        if self.compaction_in_flight && commands.iter().any(|c| c.operation == Operation::Compact) {
            return Err(io::Error::other(
                "finish background compaction before explicit compact",
            ));
        }
        let result = self.execute_inner(commands);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn execute_inner(&mut self, commands: &[Command]) -> io::Result<Vec<Response>> {
        let mut records = Vec::new();
        let mut responses = Vec::with_capacity(commands.len());
        for command in commands {
            self.state.set_now(Instant::now());
            let bytes = encode_command(command)?;
            let (response, change) = match self.state.prepare(command) {
                Ok(result) => result,
                Err(fault) => {
                    responses.push(Response::Fault(fault));
                    continue;
                }
            };
            if let Some(change) = change {
                self.sequence = self
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| invalid("journal sequence exhausted"))?;
                records.push((self.sequence, bytes));
                // Private speculative state validates the next command against
                // all predecessors. On IO failure the entire engine closes.
                self.state.apply(command.clone(), response.clone(), change);
                self.since_compaction += 1;
            }
            responses.push(response);
        }
        #[cfg(feature = "fault-injection")]
        {
            self.journal.skip_barrier_for_test = self.early_ack
                && commands
                    .iter()
                    .any(|c| matches!(c.operation, Operation::Commit { .. }));
        }
        self.journal.append_batch(&records)?;
        if commands.iter().any(|c| c.operation == Operation::Compact) {
            self.journal
                .compact(self.sequence, &self.state.snapshot()?)?;
            self.since_compaction = 0;
        }
        Ok(responses)
    }

    /// Run between batches, after delivering replies already covered by the
    /// journal barrier. Compaction failure still closes this engine completely.
    pub fn maintain(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("store failed; restart required"));
        }
        if self.compaction_in_flight {
            return Err(io::Error::other("background compaction still in flight"));
        }
        if self.compact_every != 0 && self.since_compaction >= self.compact_every {
            let result = self
                .state
                .snapshot()
                .and_then(|snapshot| self.journal.compact(self.sequence, &snapshot));
            if result.is_err() {
                self.failed = true;
            }
            result?;
            self.since_compaction = 0;
        }
        Ok(())
    }

    /// Rotate the durable prefix and return immutable work for a caller-owned
    /// worker. New commits append to a separate, directory-synced journal.
    pub fn begin_maintenance(&mut self) -> io::Result<Option<Compaction>> {
        if self.failed {
            return Err(io::Error::other("store failed; restart required"));
        }
        if self.compaction_in_flight
            || self.compact_every == 0
            || self.since_compaction < self.compact_every
        {
            return Ok(None);
        }
        let result = self
            .state
            .snapshot()
            .and_then(|snapshot| self.journal.rotate(self.sequence, snapshot));
        match result {
            Ok(work) => {
                self.compaction_in_flight = true;
                self.since_compaction = 0;
                Ok(Some(work))
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    /// Must receive the worker result before starting another compaction.
    /// A failed installation closes all engine operations, including reads.
    pub fn complete_maintenance(&mut self, result: io::Result<CompactionReady>) -> io::Result<()> {
        if !self.compaction_in_flight {
            return Err(io::Error::other("no background compaction in flight"));
        }
        let result = result.and_then(|ready| self.journal.complete_compaction(ready));
        self.compaction_in_flight = false;
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    //! Owns same-batch validation and whole-engine barrier failure checks.

    use super::*;
    use crate::{RequestId, StoreFault};
    use goliath_model::{Epoch, Fence, ProcessId, TileId};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn directory() -> std::path::PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../.local/runs/e9-batch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        path
    }

    #[test]
    fn batch_validates_predecessors_receipts_and_reads_in_queue_order() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        let old = Fence {
            tile: TileId(0),
            epoch: Epoch(1),
            process: ProcessId(1),
        };
        let fresh = Fence {
            epoch: Epoch(2),
            process: ProcessId(2),
            ..old
        };
        let commands = vec![
            Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            },
            Command {
                id: RequestId(2),
                operation: Operation::Claim(TileId(0)),
            },
            Command {
                id: RequestId(3),
                operation: Operation::Heartbeat {
                    fence: old,
                    counter: 1,
                },
            },
            Command {
                id: RequestId(4),
                operation: Operation::Heartbeat {
                    fence: fresh,
                    counter: 2,
                },
            },
            Command {
                id: RequestId(4),
                operation: Operation::Heartbeat {
                    fence: fresh,
                    counter: 2,
                },
            },
            Command {
                id: RequestId(4),
                operation: Operation::Heartbeat {
                    fence: fresh,
                    counter: 3,
                },
            },
            Command {
                id: RequestId(0),
                operation: Operation::Directory,
            },
        ];
        let responses = engine.execute_batch(&commands).unwrap();
        assert_eq!(engine.journal.barriers, 1);
        assert_eq!(
            responses[..6],
            [
                Response::Claimed(old),
                Response::Claimed(fresh),
                Response::Fault(StoreFault::StaleFence),
                Response::Ok,
                Response::Ok,
                Response::Fault(StoreFault::RequestConflict)
            ]
        );
        assert!(
            matches!(&responses[6], Response::Directory(entries) if entries[0].heartbeat == 2 && entries[0].fence == fresh)
        );
        drop(engine);
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        assert_eq!(engine.execute(&commands[6]).unwrap(), responses[6]);
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_batch_barrier_returns_no_responses_and_closes_reads_too() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        engine.journal.fail_barrier = true;
        let claim = Command {
            id: RequestId(1),
            operation: Operation::Claim(TileId(0)),
        };
        assert!(engine.execute_batch(&[claim.clone(), claim]).is_err());
        assert!(
            engine
                .execute(&Command {
                    id: RequestId(0),
                    operation: Operation::Directory
                })
                .is_err()
        );
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn background_snapshot_keeps_newer_commits_and_receipts() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
        let first = Command {
            id: RequestId(1),
            operation: Operation::Claim(TileId(0)),
        };
        let second = Command {
            id: RequestId(2),
            operation: Operation::Claim(TileId(1)),
        };
        let original = engine.execute_batch(std::slice::from_ref(&first)).unwrap();
        let work = engine.begin_maintenance().unwrap().unwrap();
        assert!(engine.begin_maintenance().unwrap().is_none());
        let newer = engine.execute_batch(std::slice::from_ref(&second)).unwrap();
        engine.complete_maintenance(work.run()).unwrap();
        assert!(!path.join("journal.previous").exists());
        assert!(fs::metadata(path.join("journal")).unwrap().len() > 0);
        drop(engine);
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        assert_eq!(engine.execute_batch(&[first]).unwrap(), original);
        assert_eq!(engine.execute_batch(&[second]).unwrap(), newer);
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn interrupted_rotation_recovers_before_and_after_snapshot_install() {
        for stage in 0..5 {
            let path = directory();
            let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
            let first = Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            };
            let second = Command {
                id: RequestId(2),
                operation: Operation::Claim(TileId(1)),
            };
            let original = engine.execute_batch(std::slice::from_ref(&first)).unwrap();
            let work = engine.begin_maintenance().unwrap().unwrap();
            if stage == 0 {
                // Cut between renaming the prefix and installing a new journal.
                fs::remove_file(path.join("journal")).unwrap();
                drop(work);
                drop(engine);
                let mut recovered = Engine::open(&path, SyncMode::Full, 0).unwrap();
                assert_eq!(recovered.execute_batch(&[first]).unwrap(), original);
                drop(recovered);
            } else {
                let newer = engine.execute_batch(std::slice::from_ref(&second)).unwrap();
                if stage >= 2 {
                    // Install snapshot but emulate a cut before prefix deletion.
                    let prefix = fs::read(path.join("journal.previous")).unwrap();
                    work.run().unwrap();
                    fs::write(path.join("journal.previous"), prefix).unwrap();
                    if stage == 3 {
                        // Unacknowledged active tail must not damage the prefix.
                        use std::io::Write;
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(path.join("journal"))
                            .unwrap()
                            .write_all(b"torn")
                            .unwrap();
                    }
                    if stage == 4 {
                        // Cut during startup cleanup, after a full recovered
                        // snapshot and journal truncation, before prefix removal.
                        let prefix = fs::read(path.join("journal.previous")).unwrap();
                        engine
                            .journal
                            .compact(engine.sequence, &engine.state.snapshot().unwrap())
                            .unwrap();
                        fs::write(path.join("journal.previous"), prefix).unwrap();
                    }
                } else {
                    drop(work);
                }
                drop(engine);
                let mut recovered = Engine::open(&path, SyncMode::Full, 0).unwrap();
                assert_eq!(recovered.execute_batch(&[first]).unwrap(), original);
                assert_eq!(recovered.execute_batch(&[second]).unwrap(), newer);
                assert!(!path.join("journal.previous").exists());
                drop(recovered);
            }
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn compaction_failure_closes_engine_and_in_flight_explicit_compact_is_rejected() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
        engine
            .execute_batch(&[Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            }])
            .unwrap();
        let work = engine.begin_maintenance().unwrap().unwrap();
        assert!(
            engine
                .execute_batch(&[Command {
                    id: RequestId(0),
                    operation: Operation::Compact
                }])
                .is_err()
        );
        // A failed snapshot write must close reads as well as mutations.
        fs::create_dir(path.join("snapshot.next")).unwrap();
        assert!(engine.complete_maintenance(work.run()).is_err());
        assert!(
            engine
                .execute_batch(&[Command {
                    id: RequestId(0),
                    operation: Operation::Directory
                }])
                .is_err()
        );
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_rotation_closes_reads_and_corrupt_rotated_prefix_refuses_startup() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
        engine
            .execute_batch(&[Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            }])
            .unwrap();
        fs::create_dir(path.join("journal.previous")).unwrap();
        assert!(engine.begin_maintenance().is_err());
        assert!(
            engine
                .execute_batch(&[Command {
                    id: RequestId(0),
                    operation: Operation::Directory
                }])
                .is_err()
        );
        drop(engine);
        fs::remove_dir(path.join("journal.previous")).unwrap();
        let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
        drop(engine.begin_maintenance().unwrap().unwrap());
        drop(engine);
        let mut prefix = fs::read(path.join("journal.previous")).unwrap();
        prefix[0] ^= 1;
        fs::write(path.join("journal.previous"), prefix).unwrap();
        assert!(Engine::open(&path, SyncMode::Full, 0).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}
