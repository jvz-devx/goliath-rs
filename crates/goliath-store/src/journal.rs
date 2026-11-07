//! Owns checksummed append records, prefix recovery and atomic snapshot compaction.

use crate::codec::invalid;
use crate::durable::{SyncMode, sync_directory};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"GSTLOG01";
const HEADER: usize = 32;
const MAX_RECORD: usize = 1024 * 1024;

pub(crate) struct Journal {
    directory: PathBuf,
    log: File,
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
}

pub(crate) struct Recovery {
    pub(crate) snapshot: Option<(u64, Vec<u8>)>,
    pub(crate) records: Vec<(u64, Vec<u8>)>,
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

impl Journal {
    pub(crate) fn open(directory: &Path, mode: SyncMode) -> io::Result<(Self, Recovery)> {
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
        let snapshot = match fs::read(directory.join("snapshot")) {
            Ok(bytes) => {
                let (sequence, payload, end) = parse(&bytes, false)
                    .ok_or_else(|| invalid("snapshot checksum/header failure"))?;
                if end != bytes.len() {
                    return Err(invalid("snapshot trailing bytes"));
                }
                Some((sequence, payload.to_vec()))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let mut log = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("journal"))?;
        mode.sync(&log)?;
        sync_directory(directory, mode)?;
        let bytes = fs::read(directory.join("journal"))?;
        let mut position = 0;
        let mut records = Vec::new();
        while position < bytes.len() {
            if let Some((sequence, payload, count)) = parse(&bytes[position..], true) {
                records.push((sequence, payload.to_vec()));
                position += count;
            } else {
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
                log.set_len(position as u64)?;
                mode.sync(&log)?;
                break;
            }
        }
        log.seek(SeekFrom::End(0))?;
        Ok((
            Self {
                directory: directory.to_path_buf(),
                log,
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
            },
            Recovery { snapshot, records },
        ))
    }

    pub(crate) fn append_batch(&mut self, records: &[(u64, Vec<u8>)]) -> io::Result<()> {
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
        self.mode.sync(&self.log)?;
        #[cfg(test)]
        {
            self.barriers += 1;
        }
        Ok(())
    }

    pub(crate) fn compact(&mut self, sequence: u64, snapshot: &[u8]) -> io::Result<()> {
        let next_path = self.directory.join("snapshot.next");
        let mut next = File::create(&next_path)?;
        next.write_all(&frame(sequence, snapshot)?)?;
        self.mode.sync(&next)?;
        fs::rename(&next_path, self.directory.join("snapshot"))?;
        sync_directory(&self.directory, self.mode)?;
        // A crash before truncation leaves the old journal. Recovery skips its
        // already-snapshotted sequence numbers. A crash after it uses snapshot.
        self.log.set_len(0)?;
        self.log.seek(SeekFrom::Start(0))?;
        self.mode.sync(&self.log)
    }
}

#[cfg(test)]
mod tests {
    //! Owns checksum coverage of every record byte.

    use super::*;

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
