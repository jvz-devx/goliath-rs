//! Owns durable-before-visible operation ordering and fail-closed IO handling.

use crate::codec::{decode_command, encode_command, invalid};
use crate::journal::Journal;
use crate::state::State;
use crate::{Command, Operation, Response, SyncMode};
use std::io;
use std::path::Path;

pub struct Engine {
    journal: Journal,
    state: State,
    sequence: u64,
    since_compaction: u64,
    compact_every: u64,
    failed: bool,
    #[cfg(feature = "fault-injection")]
    early_ack: bool,
}

impl Engine {
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
        Ok(Self {
            journal,
            state,
            sequence,
            since_compaction,
            compact_every,
            failed: false,
            #[cfg(feature = "fault-injection")]
            early_ack: false,
        })
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
}
