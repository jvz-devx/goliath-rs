//! Owns checksummed append records, prefix recovery and atomic snapshot compaction.

use crate::codec::invalid;
use crate::durable::{SyncMode, sync_directory};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAGIC: &[u8; 8] = b"GSTLOG01";
const HEADER: usize = 32;
const MAX_RECORD: usize = 1024 * 1024;

pub(crate) struct Journal {
    pub(crate) last_barrier: Duration,
    pub(crate) last_barrier_span: Option<(Instant, Instant)>,
    directory: PathBuf,
    log: File,
    prepared: Option<File>,
    // The stable lock inode is never replaced during compaction.
    _lock: File,
    mode: SyncMode,
    #[cfg(feature = "fault-injection")]
    pub(crate) skip_barrier_for_test: bool,
    #[cfg(feature = "fault-injection")]
    pub(crate) volatile_file_cache: Option<Vec<u8>>,
    #[cfg(test)]
    pub(crate) fail_barrier: bool,
    #[cfg(test)]
    pub(crate) barriers: u64,
    // Holds a barrier open until the test lets it finish.
    #[cfg(test)]
    pub(crate) barrier_gate: Option<BarrierGate>,
    // Slow storage for tests. It delays the barrier and removes none.
    #[cfg(any(test, feature = "fault-injection"))]
    pub(crate) barrier_delay: Duration,
}

/// A barrier that tells the test it has started and waits for its release.
#[cfg(test)]
pub(crate) struct BarrierGate {
    pub(crate) entered: std::sync::mpsc::Sender<()>,
    pub(crate) release: std::sync::mpsc::Receiver<()>,
}

pub(crate) struct Recovery {
    pub(crate) snapshot: Option<(u64, Vec<u8>)>,
    pub(crate) records: Vec<(u64, Vec<u8>)>,
}

/// Owns one immutable snapshot installation. The caller supplies its worker.
pub struct Compaction {
    directory: PathBuf,
    mode: SyncMode,
    sequence: u64,
    snapshot: Vec<u8>,
}

/// Completed snapshot installation plus a fully synced empty next journal.
pub struct CompactionReady {
    directory: PathBuf,
    next: File,
    snapshot: Vec<u8>,
}

impl Compaction {
    pub fn snapshot_usage(&self) -> (usize, usize) {
        (self.snapshot.len(), self.snapshot.capacity())
    }
    pub fn run(self) -> io::Result<CompactionReady> {
        install_snapshot(&self.directory, self.mode, self.sequence, &self.snapshot)?;
        fs::remove_file(self.directory.join("journal.previous"))?;
        sync_directory(&self.directory, self.mode)?;
        let next = prepare_journal(&self.directory, self.mode)?;
        Ok(CompactionReady {
            directory: self.directory,
            next,
            snapshot: self.snapshot,
        })
    }
}

fn prepare_journal(directory: &Path, mode: SyncMode) -> io::Result<File> {
    let next = File::create(directory.join("journal.next"))?;
    mode.sync(&next)?;
    Ok(next)
}

fn install_snapshot(
    directory: &Path,
    mode: SyncMode,
    sequence: u64,
    snapshot: &[u8],
) -> io::Result<()> {
    let next_path = directory.join("snapshot.next");
    let mut next = File::create(&next_path)?;
    write_frame(&mut next, sequence, snapshot)?;
    mode.sync(&next)?;
    fs::rename(&next_path, directory.join("snapshot"))?;
    sync_directory(directory, mode)
}

fn crc(bytes: impl Iterator<Item = u8>) -> u32 {
    let mut value = !0_u32;
    for byte in bytes {
        value ^= u32::from(byte);
        for _ in 0..8 {
            value = (value >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(value & 1));
        }
    }
    !value
}

fn frame(sequence: u64, bytes: &[u8]) -> io::Result<Vec<u8>> {
    let len = u32::try_from(bytes.len()).map_err(|_| invalid("record too large"))?;
    let checksum = crc(sequence
        .to_le_bytes()
        .into_iter()
        .chain(len.to_le_bytes())
        .chain(bytes.iter().copied()));
    let mut record = Vec::with_capacity(HEADER + bytes.len());
    record.extend(MAGIC);
    record.extend(sequence.to_le_bytes());
    record.extend(len.to_le_bytes());
    record.extend((!len).to_le_bytes());
    record.extend(checksum.to_le_bytes());
    record.extend((!checksum).to_le_bytes());
    record.extend(bytes);
    Ok(record)
}

/// Snapshot-only streamed representation, byte-compatible with `frame`.
/// The destination is private until the original sync/rename sequence completes.
fn write_frame(writer: &mut impl Write, sequence: u64, bytes: &[u8]) -> io::Result<()> {
    let len = u32::try_from(bytes.len()).map_err(|_| invalid("record too large"))?;
    let checksum = crc(sequence
        .to_le_bytes()
        .into_iter()
        .chain(len.to_le_bytes())
        .chain(bytes.iter().copied()));
    let mut header = [0u8; HEADER];
    header[..8].copy_from_slice(MAGIC);
    header[8..16].copy_from_slice(&sequence.to_le_bytes());
    header[16..20].copy_from_slice(&len.to_le_bytes());
    header[20..24].copy_from_slice(&(!len).to_le_bytes());
    header[24..28].copy_from_slice(&checksum.to_le_bytes());
    header[28..32].copy_from_slice(&(!checksum).to_le_bytes());
    writer.write_all(&header)?;
    writer.write_all(bytes)
}

fn parse(bytes: &[u8], bounded: bool) -> Option<(u64, &[u8], usize)> {
    if bytes.len() < HEADER || &bytes[..8] != MAGIC {
        return None;
    }
    let sequence = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
    let len = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    let complement = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    let checksum = u32::from_le_bytes(bytes[24..28].try_into().ok()?);
    let check_complement = u32::from_le_bytes(bytes[28..32].try_into().ok()?);
    if complement != !len || check_complement != !checksum || (bounded && len as usize > MAX_RECORD)
    {
        return None;
    }
    let end = HEADER.checked_add(len as usize)?;
    let payload = bytes.get(HEADER..end)?;
    let computed = crc(sequence
        .to_le_bytes()
        .into_iter()
        .chain(len.to_le_bytes())
        .chain(payload.iter().copied()));
    (computed == checksum).then_some((sequence, payload, end))
}

fn read_snapshot(directory: &Path) -> io::Result<Option<(u64, Vec<u8>)>> {
    match fs::read(directory.join("snapshot")) {
        Ok(bytes) => {
            let (sequence, payload, end) =
                parse(&bytes, false).ok_or_else(|| invalid("snapshot checksum/header failure"))?;
            if end != bytes.len() {
                return Err(invalid("snapshot trailing bytes"));
            }
            Ok(Some((sequence, payload.to_vec())))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl Journal {
    pub(crate) fn open(
        directory: &Path,
        mode: SyncMode,
        mut validate_snapshot: impl FnMut(Option<&[u8]>) -> io::Result<()>,
    ) -> io::Result<(Self, Recovery)> {
        // Refuse unsupported history before creating files or repairing a tail.
        // Revalidate under the writer lock if an existing writer changed it.
        let inspected = read_snapshot(directory)?;
        validate_snapshot(inspected.as_ref().map(|(_, bytes)| bytes.as_slice()))?;
        match fs::create_dir(directory) {
            Ok(()) => sync_directory(directory.parent().unwrap_or(Path::new(".")), mode)?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("writer.lock"))?;
        lock.try_lock().map_err(io::Error::from)?;
        let snapshot = read_snapshot(directory)?;
        if snapshot != inspected {
            validate_snapshot(snapshot.as_ref().map(|(_, bytes)| bytes.as_slice()))?;
        }
        drop(inspected);
        let mut log = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("journal"))?;
        mode.sync(&log)?;
        sync_directory(directory, mode)?;
        // A rotated durable prefix remains until its snapshot is installed.
        // Replaying it before the active journal covers every rotation cut.
        let previous_path = directory.join("journal.previous");
        let previous_bytes = match fs::read(&previous_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        let active_bytes = fs::read(directory.join("journal"))?;
        let previous_len = previous_bytes.len();
        let mut bytes = previous_bytes;
        bytes.extend(active_bytes);
        let mut position = 0;
        let mut records = Vec::new();
        while position < bytes.len() {
            if let Some((sequence, payload, count)) = parse(&bytes[position..], true) {
                records.push((sequence, payload.to_vec()));
                position += count;
            } else {
                if position < previous_len {
                    return Err(invalid("corrupt rotated journal prefix"));
                }
                // Never hide a corrupt interior record by dropping valid successors.
                let has_successor =
                    bytes[position + 1..]
                        .windows(8)
                        .enumerate()
                        .any(|(i, magic)| {
                            magic == MAGIC && parse(&bytes[position + 1 + i..], true).is_some()
                        });
                if has_successor {
                    return Err(invalid("interior journal corruption"));
                }
                log.set_len((position - previous_len) as u64)?;
                mode.sync(&log)?;
                break;
            }
        }
        log.seek(SeekFrom::End(0))?;
        let prepared = Some(prepare_journal(directory, mode)?);
        Ok((
            Self {
                last_barrier: Duration::ZERO,
                last_barrier_span: None,
                directory: directory.to_path_buf(),
                log,
                prepared,
                _lock: lock,
                mode,
                #[cfg(feature = "fault-injection")]
                skip_barrier_for_test: false,
                #[cfg(feature = "fault-injection")]
                volatile_file_cache: None,
                #[cfg(test)]
                fail_barrier: false,
                #[cfg(test)]
                barriers: 0,
                #[cfg(test)]
                barrier_gate: None,
                #[cfg(any(test, feature = "fault-injection"))]
                barrier_delay: Duration::ZERO,
            },
            Recovery { snapshot, records },
        ))
    }

    pub(crate) fn append_batch(&mut self, records: &[(u64, Vec<u8>)]) -> io::Result<()> {
        self.last_barrier = Duration::ZERO;
        self.last_barrier_span = None;
        if records.is_empty() {
            return Ok(());
        }
        let mut buffer = Vec::new();
        for (sequence, bytes) in records {
            if bytes.len() > MAX_RECORD {
                return Err(invalid("journal record too large"));
            }
            buffer.extend(frame(*sequence, bytes)?);
        }
        #[cfg(feature = "fault-injection")]
        if let Some(cache) = &mut self.volatile_file_cache {
            // Fault-injecting file layer: writes reach only volatile cache.
            // The early-ack mutation omits the barrier that would persist them.
            // Process death drops this cache, modeling loss of unflushed writes.
            cache.extend(buffer);
            if self.skip_barrier_for_test {
                return Ok(());
            }
            buffer = std::mem::take(cache);
        }
        self.log.write_all(&buffer)?;
        #[cfg(test)]
        if self.fail_barrier {
            return Err(io::Error::other("injected barrier failure"));
        }
        let barrier_start = Instant::now();
        #[cfg(any(test, feature = "fault-injection"))]
        std::thread::sleep(self.barrier_delay);
        #[cfg(test)]
        if let Some(gate) = &self.barrier_gate {
            gate.entered.send(()).map_err(io::Error::other)?;
            gate.release.recv().map_err(io::Error::other)?;
        }
        self.mode.sync(&self.log)?;
        let barrier_end = Instant::now();
        self.last_barrier = barrier_end.duration_since(barrier_start);
        self.last_barrier_span = Some((barrier_start, barrier_end));
        #[cfg(test)]
        {
            self.barriers += 1;
        }
        Ok(())
    }

    pub(crate) fn compact(&mut self, sequence: u64, snapshot: &[u8]) -> io::Result<()> {
        install_snapshot(&self.directory, self.mode, sequence, snapshot)?;
        // A crash before truncation leaves the old journal. Recovery skips its
        // already-snapshotted sequence numbers. A crash after it uses snapshot.
        self.log.set_len(0)?;
        self.log.seek(SeekFrom::Start(0))?;
        self.mode.sync(&self.log)?;
        if self.has_rotation()? {
            fs::remove_file(self.directory.join("journal.previous"))?;
            sync_directory(&self.directory, self.mode)?;
        }
        Ok(())
    }

    pub(crate) fn has_rotation(&self) -> io::Result<bool> {
        self.directory.join("journal.previous").try_exists()
    }

    pub(crate) fn rotate(&mut self, sequence: u64, snapshot: Vec<u8>) -> io::Result<Compaction> {
        let next_path = self.directory.join("journal.next");
        let next = self
            .prepared
            .take()
            .ok_or_else(|| invalid("no prepared journal"))?;
        fs::rename(
            self.directory.join("journal"),
            self.directory.join("journal.previous"),
        )?;
        fs::rename(next_path, self.directory.join("journal"))?;
        sync_directory(&self.directory, self.mode)?;
        self.log = next;
        Ok(Compaction {
            directory: self.directory.clone(),
            mode: self.mode,
            sequence,
            snapshot,
        })
    }

    pub(crate) fn complete_compaction(&mut self, ready: CompactionReady) -> io::Result<Vec<u8>> {
        if ready.directory != self.directory || self.prepared.is_some() {
            return Err(invalid("wrong compaction completion"));
        }
        self.prepared = Some(ready.next);
        Ok(ready.snapshot)
    }
}

#[cfg(test)]
mod tests {
    //! Owns checksum coverage of every record byte.

    use super::*;

    #[test]
    fn streamed_snapshot_is_byte_identical_to_the_original_full_frame() {
        for length in [0, 1, 1023, 1024 * 1024] {
            let payload: Vec<_> = (0..length).map(|index| (index % 251) as u8).collect();
            for sequence in [0, 7, u64::MAX] {
                let expected = frame(sequence, &payload).unwrap();
                let mut streamed = Vec::new();
                write_frame(&mut streamed, sequence, &payload).unwrap();
                assert_eq!(streamed, expected);
            }
        }
    }

    #[test]
    fn recovery_ignores_a_crash_after_header_or_after_synced_write_before_rename() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../.local/runs/e24-stream-crash-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let (mut journal, _) = Journal::open(&path, SyncMode::Full, |_| Ok(())).unwrap();
        journal.compact(7, b"old committed snapshot").unwrap();
        drop(journal);
        struct HeaderOnly {
            file: File,
            left: usize,
        }
        impl Write for HeaderOnly {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.left == 0 {
                    return Err(io::Error::other("injected crash after header"));
                }
                let count = bytes.len().min(self.left);
                let written = self.file.write(&bytes[..count])?;
                self.left -= written;
                Ok(written)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.file.flush()
            }
        }
        let mut interrupted = HeaderOnly {
            file: File::create(path.join("snapshot.next")).unwrap(),
            left: HEADER,
        };
        assert!(write_frame(&mut interrupted, 8, b"new uncommitted snapshot").is_err());
        SyncMode::Full.sync(&interrupted.file).unwrap();
        drop(interrupted);
        let (journal, recovered) = Journal::open(&path, SyncMode::Full, |_| Ok(())).unwrap();
        assert_eq!(
            recovered.snapshot,
            Some((7, b"old committed snapshot".to_vec()))
        );
        drop(journal);
        let mut next = File::create(path.join("snapshot.next")).unwrap();
        write_frame(&mut next, 8, b"new fully written but not renamed").unwrap();
        SyncMode::Full.sync(&next).unwrap();
        drop(next);
        let (journal, recovered) = Journal::open(&path, SyncMode::Full, |_| Ok(())).unwrap();
        assert_eq!(
            recovered.snapshot,
            Some((7, b"old committed snapshot".to_vec()))
        );
        drop(journal);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn every_header_payload_and_checksum_byte_is_protected() {
        let bytes = frame(7, b"payload").unwrap();
        assert_eq!(parse(&bytes, true).unwrap().0, 7);
        for offset in 0..bytes.len() {
            let mut corrupt = bytes.clone();
            corrupt[offset] ^= 1;
            assert!(parse(&corrupt, true).is_none(), "offset {offset}");
            assert!(parse(&bytes[..offset], true).is_none());
        }
    }
}
