//! Owns durable-before-visible operation ordering and fail-closed IO handling.

use crate::codec::{decode_command, encode_command, invalid};
use crate::journal::{Compaction, CompactionReady, Journal};
use crate::state::State;
use crate::{Command, Operation, Response, SyncMode};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub struct Engine {
    journal: Journal,
    // One lock orders every command and every early renewal. It is never
    // held across journal IO, so a renewal does not wait for a barrier.
    state: Arc<Mutex<State>>,
    sequence: u64,
    since_compaction: u64,
    compact_every: u64,
    failed: bool,
    compaction_in_flight: bool,
    snapshot_buffer: Vec<u8>,
    #[cfg(feature = "fault-injection")]
    early_ack: bool,
}

/// Sampled ownership sizes, not allocator RSS. Blob counters exclude map nodes.
pub struct StoreResources {
    pub accepted: u64,
    pub receipts: usize,
    pub receipt_blob_bytes: usize,
    pub receipt_blob_capacity_bytes: usize,
    pub receipt_inline_bytes: usize,
    pub players: usize,
    pub uuid_players: usize,
    pub player_blob_bytes: usize,
    pub player_blob_capacity_bytes: usize,
    pub snapshot_len: usize,
    pub snapshot_capacity: usize,
    pub in_flight: bool,
}

/// Answers granted lease renewals without the journal or its barrier.
///
/// The handle is cheap to clone and owns no thread. The caller decides which
/// of its threads ask. A renewal answered here is not journaled: a crash loses
/// it, and the restarted store's full-lease wait covers the lost grant.
#[derive(Clone)]
pub struct Renewer {
    state: Arc<Mutex<State>>,
}

impl Renewer {
    /// `Some` is the complete reply to a `RenewLease`. `None` means the
    /// command must go through `Engine::execute_batch` unchanged: it is not a
    /// renewal, it would be refused, it is a retry the journal remembers, or
    /// its lease is not yet on stable storage.
    pub fn renew(&self, command: &Command) -> Option<Response> {
        if !matches!(command.operation, Operation::RenewLease { .. }) {
            return None;
        }
        // A poisoned lock means a thread died inside the state. Fail closed.
        let mut state = self.state.lock().ok()?;
        state.renew_unjournaled(command, Instant::now())
    }
}

fn poisoned<T>(_: PoisonError<T>) -> io::Error {
    io::Error::other("store state poisoned; restart required")
}

impl Engine {
    /// The early-renewal path over this engine's state.
    pub fn renewer(&self) -> Renewer {
        Renewer {
            state: Arc::clone(&self.state),
        }
    }

    // Statistics only. Decisions use `lock`, which fails closed on poison.
    fn peek(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.state.lock().map_err(poisoned)
    }

    fn fail(&mut self) {
        self.failed = true;
        self.peek().close();
    }

    pub fn accepted_count(&self) -> u64 {
        self.sequence
    }

    pub fn receipt_count(&self) -> usize {
        self.peek().receipt_count()
    }

    pub fn resources(&self) -> StoreResources {
        let state = self.peek();
        let (bytes, capacity) = state.receipt_blob_bytes();
        let (players, uuid_players, player_blob_bytes, player_blob_capacity_bytes) =
            state.player_blob_stats();
        let receipts = state.receipt_count();
        drop(state);
        StoreResources {
            accepted: self.sequence,
            receipts,
            receipt_blob_bytes: bytes,
            receipt_blob_capacity_bytes: capacity,
            receipt_inline_bytes: receipts * std::mem::size_of::<(Command, Response)>(),
            players,
            uuid_players,
            player_blob_bytes,
            player_blob_capacity_bytes,
            snapshot_len: self.snapshot_buffer.len(),
            snapshot_capacity: self.snapshot_buffer.capacity(),
            in_flight: self.compaction_in_flight,
        }
    }

    pub fn last_barrier_span(&self) -> Option<(Instant, Instant)> {
        self.journal.last_barrier_span
    }

    fn capture_snapshot(&mut self) -> io::Result<Vec<u8>> {
        let buffer = std::mem::take(&mut self.snapshot_buffer);
        // Early renewals wait while the state is serialized. No IO happens here.
        let state = self.lock()?;
        if buffer.capacity() == 0 {
            state.snapshot()
        } else {
            state.snapshot_into(buffer)
        }
    }

    fn compact_now(&mut self) -> io::Result<()> {
        let snapshot = self.capture_snapshot()?;
        let result = self.journal.compact(self.sequence, &snapshot);
        self.snapshot_buffer = snapshot;
        result
    }
    /// Duration of the last batch's journal barrier, excluding encoding and append.
    pub fn last_barrier_duration(&self) -> Duration {
        self.journal.last_barrier
    }

    pub fn open(directory: &Path, mode: SyncMode, compact_every: u64) -> io::Result<Self> {
        let mut state = State::default();
        let (journal, recovery) = Journal::open(directory, mode, |snapshot| {
            state = match snapshot {
                Some(bytes) => State::restore(bytes)?,
                None => State::default(),
            };
            Ok(())
        })?;
        let mut sequence = recovery.snapshot.map_or(0, |(sequence, _)| sequence);
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
            state: Arc::new(Mutex::new(state)),
            sequence,
            since_compaction,
            compact_every,
            failed: false,
            compaction_in_flight: false,
            snapshot_buffer: Vec::new(),
            #[cfg(feature = "fault-injection")]
            early_ack: false,
        };
        // Finish an interrupted rotation from recovered state, before serving.
        // Never consolidate overlapping log files by copying their bytes.
        if engine.journal.has_rotation()? {
            engine.compact_now()?;
            engine.since_compaction = 0;
        }
        // Historical commands replay without today's deadlines. Only after all
        // recovery IO completes does every durable lease get a full-L quarantine.
        engine.lock()?.restart_leases()?;
        Ok(engine)
    }

    #[cfg(feature = "fault-injection")]
    pub fn disable_fence_for_test(&mut self) {
        self.peek().disable_fence = true;
    }

    #[cfg(feature = "fault-injection")]
    pub fn acknowledge_before_barrier_for_test(&mut self) {
        self.early_ack = true;
        self.power_loss_file_layer_for_test();
    }

    /// Slow storage only: every barrier still happens, later.
    #[cfg(feature = "fault-injection")]
    pub fn delay_barrier_for_test(&mut self, delay: Duration) {
        self.journal.barrier_delay = delay;
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
            self.fail();
        }
        result
    }

    fn execute_inner(&mut self, commands: &[Command]) -> io::Result<Vec<Response>> {
        let mut records = Vec::new();
        let mut responses = Vec::with_capacity(commands.len());
        for command in commands {
            let bytes = encode_command(command)?;
            // Read the clock, prepare and apply under one hold of the lock.
            // An early renewal lands wholly before or wholly after a command.
            let mut state = self.state.lock().map_err(poisoned)?;
            state.set_now(Instant::now());
            let (response, change) = match state.prepare(command) {
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
                state.apply(command.clone(), response.clone(), change);
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
        // Every claim applied above is now durable and may be renewed early.
        self.lock()?.settle();
        if commands.iter().any(|c| c.operation == Operation::Compact) {
            self.compact_now()?;
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
            let result = self.compact_now();
            if result.is_err() {
                self.fail();
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
            .capture_snapshot()
            .and_then(|snapshot| self.journal.rotate(self.sequence, snapshot));
        match result {
            Ok(work) => {
                self.compaction_in_flight = true;
                self.since_compaction = 0;
                Ok(Some(work))
            }
            Err(error) => {
                self.fail();
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
        match result {
            Ok(buffer) => {
                self.snapshot_buffer = buffer;
                Ok(())
            }
            Err(error) => {
                self.fail();
                Err(error)
            }
        }
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

    #[test]
    fn completed_snapshot_allocation_is_reused_by_the_next_compaction() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 1).unwrap();
        engine.snapshot_buffer = Vec::with_capacity(256 * 1024);
        let original = engine.snapshot_buffer.as_ptr();
        let response = engine
            .execute_batch(&[Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            }])
            .unwrap();
        let Response::Claimed(fence) = response[0] else {
            panic!("claim expected")
        };
        for command in [
            None,
            Some(Command {
                id: RequestId(2),
                operation: Operation::Heartbeat { fence, counter: 1 },
            }),
        ] {
            if let Some(command) = command {
                engine.execute_batch(&[command]).unwrap();
            }
            let work = engine.begin_maintenance().unwrap().unwrap();
            assert_eq!(work.snapshot_usage().1, 256 * 1024);
            assert!(engine.resources().in_flight);
            engine
                .complete_maintenance(Ok(work.run().unwrap()))
                .unwrap();
            assert_eq!(engine.snapshot_buffer.as_ptr(), original);
            assert_eq!(engine.resources().snapshot_capacity, 256 * 1024);
            assert!(!engine.resources().in_flight);
        }
        let expected = engine.peek().snapshot().unwrap();
        let stored = fs::read(path.join("snapshot")).unwrap();
        assert_eq!(&stored[32..], expected);
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

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
                        let snapshot = engine.peek().snapshot().unwrap();
                        engine.journal.compact(engine.sequence, &snapshot).unwrap();
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

    #[test]
    fn e24_receipt_fifo_survives_journal_sync_and_background_compaction() {
        use crate::receipts::CAPACITY;
        for mode in 0..3 {
            let path = directory();
            let mut engine = Engine::open(&path, SyncMode::Full, u64::from(mode == 2)).unwrap();
            let original = Command {
                id: RequestId(u128::MAX),
                operation: Operation::Claim(TileId(3)),
            };
            let accepted = engine
                .execute_batch(std::slice::from_ref(&original))
                .unwrap()
                .pop()
                .unwrap();
            let fillers: Vec<_> = (0..CAPACITY - 1)
                .map(|n| Command {
                    id: RequestId(1_000_000 - n as u128),
                    operation: Operation::Claim(TileId(2)),
                })
                .collect();
            engine.execute_batch(&fillers).unwrap();
            if mode == 1 {
                engine
                    .execute_batch(&[Command {
                        id: RequestId(0),
                        operation: Operation::Compact,
                    }])
                    .unwrap();
            } else if mode == 2 {
                let work = engine.begin_maintenance().unwrap().unwrap();
                engine.complete_maintenance(work.run()).unwrap();
            }
            drop(engine);
            let mut recovered = Engine::open(&path, SyncMode::Full, 0).unwrap();
            let sequence = recovered.sequence;
            assert_eq!(recovered.execute(&original).unwrap(), accepted);
            assert_eq!(recovered.sequence, sequence);
            recovered
                .execute(&Command {
                    id: RequestId(2_000_000),
                    operation: Operation::Claim(TileId(2)),
                })
                .unwrap();
            drop(recovered);
            let mut recovered = Engine::open(&path, SyncMode::Full, 0).unwrap();
            let sequence = recovered.sequence;
            assert_ne!(recovered.execute(&original).unwrap(), accepted);
            assert_eq!(recovered.sequence, sequence + 1);
            drop(recovered);
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn e24_clock_guard_does_not_reject_already_accepted_journal_history() {
        use crate::ClusterClock;
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        let fence = Fence {
            tile: TileId(0),
            process: ProcessId(1),
            epoch: Epoch(1),
        };
        let sky = |unix_ms| ClusterClock {
            unix_ms,
            game_tick: 5,
            day_tick: 9,
            raining: false,
            thundering: false,
        };
        let commands = [
            Command {
                id: RequestId(1),
                operation: Operation::Claim(TileId(0)),
            },
            Command {
                id: RequestId(2),
                operation: Operation::SetClock {
                    fence,
                    clock: sky(20),
                },
            },
            Command {
                id: RequestId(3),
                operation: Operation::SetClock {
                    fence,
                    clock: sky(10),
                },
            },
        ];
        let records: Vec<_> = commands
            .iter()
            .enumerate()
            .map(|(n, command)| ((n + 1) as u64, encode_command(command).unwrap()))
            .collect();
        engine.journal.append_batch(&records).unwrap();
        drop(engine);
        let mut recovered = Engine::open(&path, SyncMode::Full, 0).unwrap();
        assert_eq!(
            recovered
                .execute(&Command {
                    id: RequestId(0),
                    operation: Operation::Clock
                })
                .unwrap(),
            Response::Clock(Some(sky(10)))
        );
        assert_eq!(
            recovered
                .execute(&Command {
                    id: RequestId(4),
                    operation: Operation::SetClock {
                        fence,
                        clock: sky(9)
                    }
                })
                .unwrap(),
            Response::Fault(StoreFault::RequestConflict)
        );
        drop(recovered);
        fs::remove_dir_all(path).unwrap();
    }

    fn lease_claim(id: u128, tile: usize, lease_ms: u64) -> Command {
        Command {
            id: RequestId(id),
            operation: Operation::ClaimLease {
                tile: TileId(tile),
                lease_ms,
            },
        }
    }

    fn renewal(id: u128, fence: Fence, counter: u64) -> Command {
        Command {
            id: RequestId(id),
            operation: Operation::RenewLease { fence, counter },
        }
    }

    #[test]
    fn e29_a_renewal_is_answered_inside_a_stalled_barrier_and_the_claim_waits_for_it() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        let renewer = engine.renewer();
        let second = Duration::from_millis(1000);
        let Response::Leased(grant) = engine.execute(&lease_claim(1, 0, 1000)).unwrap() else {
            panic!("lease expected")
        };
        engine.journal.barrier_delay = second;
        // The claim was granted before this instant, so it expires by start + 1 s.
        let start = Instant::now();
        let (asked, answered) = std::thread::scope(|scope| {
            // An unrelated mutation holds the engine inside its barrier.
            let worker = scope.spawn(|| {
                engine
                    .execute(&Command {
                        id: RequestId(2),
                        operation: Operation::Claim(TileId(5)),
                    })
                    .unwrap()
            });
            std::thread::sleep(Duration::from_millis(300));
            let asked = Instant::now();
            let reply = renewer.renew(&renewal(3, grant.fence, 1));
            let answered = Instant::now();
            assert_eq!(
                reply,
                Some(Response::Leased(goliath_model::LeaseGrant {
                    counter: 1,
                    ..grant
                }))
            );
            assert!(answered < start + second, "host too slow to judge");
            assert!(!worker.is_finished());
            assert!(matches!(worker.join().unwrap(), Response::Claimed(_)));
            (asked, answered)
        });
        engine.journal.barrier_delay = Duration::ZERO;
        // The barrier ended after start + 1 s: the original claim has expired.
        assert!(Instant::now() >= start + second);
        let takeover = Command {
            id: RequestId(4),
            operation: Operation::BeginTakeover(grant.fence),
        };
        assert_eq!(engine.execute(&takeover).unwrap(), Response::Ok);
        let busy = engine.execute(&lease_claim(5, 0, 1000)).unwrap();
        // Evaluated before asked + 1 s, the earliest the renewed lease can end.
        assert!(Instant::now() < asked + second, "host too slow to judge");
        assert_eq!(busy, Response::Fault(StoreFault::LeaseBusy));
        std::thread::sleep((answered + second).saturating_duration_since(Instant::now()));
        let Response::Leased(successor) = engine.execute(&lease_claim(6, 0, 1000)).unwrap() else {
            panic!("successor expected")
        };
        assert_ne!(successor.fence, grant.fence);
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn e29_recovery_replays_a_journal_that_never_saw_the_renewals() {
        use crate::receipts::CAPACITY;
        for compact in [false, true] {
            let path = directory();
            let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
            let renewer = engine.renewer();
            let claim = lease_claim(1, 0, 60_000);
            let Response::Leased(grant) = engine.execute(&claim).unwrap() else {
                panic!("lease expected")
            };
            let fence = grant.fence;
            for counter in 1..=3 {
                let reply = renewer.renew(&renewal(10 + u128::from(counter), fence, counter));
                assert!(matches!(reply, Some(Response::Leased(_))));
            }
            // Only the claim took a journal sequence.
            assert_eq!(engine.sequence, 1);
            // Journaled and unjournaled renewals interleave around a snapshot.
            let journaled = engine.execute(&renewal(20, fence, 4)).unwrap();
            assert!(matches!(journaled, Response::Leased(_)));
            assert!(renewer.renew(&renewal(21, fence, 5)).is_some());
            if compact {
                let compaction = Command {
                    id: RequestId(0),
                    operation: Operation::Compact,
                };
                engine.execute(&compaction).unwrap();
            }
            assert!(matches!(
                engine.execute(&renewal(22, fence, 6)).unwrap(),
                Response::Leased(_)
            ));
            assert_eq!(engine.sequence, 3);
            // More unjournaled renewals than the retry window holds must not
            // evict a journaled receipt, or replay would see another window.
            for n in 0..CAPACITY as u64 + 8 {
                let reply = renewer.renew(&renewal(1000 + u128::from(n), fence, 7 + n));
                assert!(matches!(reply, Some(Response::Leased(_))));
            }
            assert_eq!(engine.receipt_count(), 3);
            assert_eq!(engine.execute(&claim).unwrap(), Response::Leased(grant));
            assert_eq!(engine.sequence, 3);
            drop(engine);
            let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
            let directory = Command {
                id: RequestId(0),
                operation: Operation::Directory,
            };
            // The journal's last renewal, not the last one granted.
            assert!(matches!(
                engine.execute(&directory).unwrap(),
                Response::Directory(entries)
                    if entries[0].fence == fence && entries[0].active && entries[0].heartbeat == 6
            ));
            assert_eq!(
                engine.execute(&lease_claim(30, 0, 60_000)).unwrap(),
                Response::Fault(StoreFault::LeaseBusy)
            );
            // The holder's next counter is granted early on the recovered lease.
            let next = renewal(31, fence, CAPACITY as u64 + 100);
            assert!(matches!(
                engine.renewer().renew(&next),
                Some(Response::Leased(_))
            ));
            drop(engine);
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn e29_a_failed_journal_stops_early_renewals() {
        let path = directory();
        let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
        let renewer = engine.renewer();
        let Response::Leased(grant) = engine.execute(&lease_claim(1, 0, 60_000)).unwrap() else {
            panic!("lease expected")
        };
        assert!(renewer.renew(&renewal(2, grant.fence, 1)).is_some());
        engine.journal.fail_barrier = true;
        assert!(engine.execute(&lease_claim(3, 1, 60_000)).is_err());
        assert_eq!(renewer.renew(&renewal(4, grant.fence, 2)), None);
        // The fallback is the journaled path, which is closed too.
        assert!(engine.execute(&renewal(4, grant.fence, 2)).is_err());
        drop(engine);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn e24_unsupported_snapshot_does_not_repair_or_modify_legacy_files() {
        use crate::codec::Encoder;
        use crate::receipts::CAPACITY;
        for missing_auxiliary in [false, true] {
            let path = directory();
            let mut engine = Engine::open(&path, SyncMode::Full, 0).unwrap();
            let mut snapshot = Encoder::default();
            snapshot.u64(0);
            snapshot.count(0).unwrap();
            snapshot.count(0).unwrap();
            snapshot.count(CAPACITY + 1).unwrap();
            for n in 0..CAPACITY + 1 {
                let command = Command {
                    id: RequestId((n + 1) as u128),
                    operation: Operation::Claim(TileId(0)),
                };
                snapshot.bytes(&encode_command(&command).unwrap()).unwrap();
                snapshot
                    .bytes(&crate::codec::encode_response(&Response::Ok).unwrap())
                    .unwrap();
            }
            engine.journal.compact(0, &snapshot.finish()).unwrap();
            drop(engine);
            fs::write(path.join("journal"), b"torn").unwrap();
            if missing_auxiliary {
                fs::remove_file(path.join("writer.lock")).unwrap();
                fs::remove_file(path.join("journal.next")).unwrap();
            }
            let files = || -> std::collections::BTreeMap<_, _> {
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| {
                        let entry = entry.unwrap();
                        (entry.file_name(), fs::read(entry.path()).unwrap())
                    })
                    .collect()
            };
            let original = files();
            let error = Engine::open(&path, SyncMode::Full, 0).err().unwrap();
            assert!(error.to_string().contains("explicit migration"));
            assert_eq!(files(), original);
            fs::remove_dir_all(path).unwrap();
        }
    }
}
