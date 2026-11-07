//! Owns the versioned, size-bounded binary encoding used by TCP and the journal.

use crate::state::{
    Blob, Command, DirectoryEntry, MAX_BLOB, Operation, RequestId, Response, StoreFault,
};
use goliath_model::{Epoch, Fence, Generation, OwnershipCas, PlayerId, ProcessId, Record, TileId};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MAX_FRAME: usize = 1024 * 1024;

pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Default)]
pub(crate) struct Encoder(Vec<u8>);

impl Encoder {
    pub(crate) fn finish(self) -> Vec<u8> {
        self.0
    }
    fn byte(&mut self, value: u8) {
        self.0.push(value);
    }
    pub(crate) fn u64(&mut self, value: u64) {
        self.0.extend(value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend(value.to_le_bytes());
    }
    pub(crate) fn count(&mut self, count: usize) -> io::Result<()> {
        self.u32(u32::try_from(count).map_err(|_| invalid("count overflow"))?);
        Ok(())
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> io::Result<()> {
        self.count(value.len())?;
        self.0.extend(value);
        Ok(())
    }
    fn fence(&mut self, fence: Fence) {
        self.u64(fence.tile.0 as u64);
        self.u64(fence.process.0);
        self.u64(fence.epoch.0);
    }
    fn blob(&mut self, blob: &Blob) -> io::Result<()> {
        if blob.bytes.len() > MAX_BLOB {
            return Err(invalid("blob exceeds 256 KiB"));
        }
        self.u32(blob.version);
        self.bytes(&blob.bytes)
    }
    pub(crate) fn record(&mut self, record: &Record<Blob>) -> io::Result<()> {
        self.u64(record.generation.0);
        self.u64(record.owner.0 as u64);
        self.blob(&record.inventory)
    }
    pub(crate) fn entry(&mut self, entry: &DirectoryEntry) {
        self.fence(entry.fence);
        self.byte(u8::from(entry.active));
        self.u64(entry.heartbeat);
    }
}

pub(crate) struct Decoder<'a>(&'a [u8]);

impl<'a> Decoder<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        if count > self.0.len() {
            return Err(invalid("truncated encoding"));
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| invalid("u64"))?,
        ))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| invalid("u32"))?,
        ))
    }
    pub(crate) fn count(&mut self) -> io::Result<usize> {
        let count = self.u32()? as usize;
        if count > self.0.len() {
            return Err(invalid("count exceeds remaining bytes"));
        }
        Ok(count)
    }
    pub(crate) fn index(&mut self) -> io::Result<usize> {
        usize::try_from(self.u64()?).map_err(|_| invalid("id overflow"))
    }
    pub(crate) fn bytes(&mut self) -> io::Result<&'a [u8]> {
        let count = self.count()?;
        self.take(count)
    }
    fn fence(&mut self) -> io::Result<Fence> {
        Ok(Fence {
            tile: TileId(self.index()?),
            process: ProcessId(self.u64()?),
            epoch: Epoch(self.u64()?),
        })
    }
    fn blob(&mut self) -> io::Result<Blob> {
        let version = self.u32()?;
        let bytes = self.bytes()?;
        if bytes.len() > MAX_BLOB {
            return Err(invalid("blob exceeds 256 KiB"));
        }
        Ok(Blob {
            version,
            bytes: bytes.to_vec(),
        })
    }
    pub(crate) fn record(&mut self) -> io::Result<Record<Blob>> {
        Ok(Record {
            generation: Generation(self.u64()?),
            owner: TileId(self.index()?),
            inventory: self.blob()?,
        })
    }
    pub(crate) fn entry(&mut self) -> io::Result<DirectoryEntry> {
        let fence = self.fence()?;
        let active = match self.byte()? {
            0 => false,
            1 => true,
            _ => return Err(invalid("invalid active flag")),
        };
        Ok(DirectoryEntry {
            fence,
            active,
            heartbeat: self.u64()?,
        })
    }
    pub(crate) fn end(self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("trailing bytes"))
        }
    }
}

pub(crate) fn encode_command(command: &Command) -> io::Result<Vec<u8>> {
    let mut e = Encoder::default();
    e.byte(1);
    e.0.extend(command.id.0.to_le_bytes());
    match &command.operation {
        Operation::Claim(tile) => {
            e.byte(1);
            e.u64(tile.0 as u64);
        }
        Operation::Directory => e.byte(2),
        Operation::Read(player) => {
            e.byte(3);
            e.u64(player.0 as u64);
        }
        Operation::Create {
            player,
            owner,
            blob,
        } => {
            e.byte(4);
            e.u64(player.0 as u64);
            e.fence(*owner);
            e.blob(blob)?;
        }
        Operation::Commit { player, cas, blob } => {
            e.byte(5);
            e.u64(player.0 as u64);
            e.u64(cas.generation.0);
            e.fence(cas.source);
            e.fence(cas.target);
            e.blob(blob)?;
        }
        Operation::Release(fence) => {
            e.byte(6);
            e.fence(*fence);
        }
        Operation::Heartbeat { fence, counter } => {
            e.byte(7);
            e.fence(*fence);
            e.u64(*counter);
        }
        Operation::Compact => e.byte(8),
    }
    Ok(e.finish())
}

pub(crate) fn decode_command(bytes: &[u8]) -> io::Result<Command> {
    let mut d = Decoder::new(bytes);
    if d.byte()? != 1 {
        return Err(invalid("unsupported protocol version"));
    }
    let id = RequestId(u128::from_le_bytes(
        d.take(16)?.try_into().map_err(|_| invalid("id"))?,
    ));
    let operation = match d.byte()? {
        1 => Operation::Claim(TileId(d.index()?)),
        2 => Operation::Directory,
        3 => Operation::Read(PlayerId(d.index()?)),
        4 => Operation::Create {
            player: PlayerId(d.index()?),
            owner: d.fence()?,
            blob: d.blob()?,
        },
        5 => Operation::Commit {
            player: PlayerId(d.index()?),
            cas: OwnershipCas {
                generation: Generation(d.u64()?),
                source: d.fence()?,
                target: d.fence()?,
            },
            blob: d.blob()?,
        },
        6 => Operation::Release(d.fence()?),
        7 => Operation::Heartbeat {
            fence: d.fence()?,
            counter: d.u64()?,
        },
        8 => Operation::Compact,
        _ => return Err(invalid("unknown operation")),
    };
    d.end()?;
    Ok(Command { id, operation })
}

pub(crate) fn encode_response(response: &Response) -> io::Result<Vec<u8>> {
    let mut e = Encoder::default();
    e.byte(1);
    match response {
        Response::Ok => e.byte(0),
        Response::Claimed(fence) => {
            e.byte(1);
            e.fence(*fence);
        }
        Response::Directory(entries) => {
            e.byte(2);
            e.count(entries.len())?;
            for entry in entries {
                e.entry(entry);
            }
        }
        Response::Player(record) => {
            e.byte(3);
            e.byte(u8::from(record.is_some()));
            if let Some(record) = record {
                e.record(record)?;
            }
        }
        Response::Committed(generation) => {
            e.byte(4);
            e.u64(generation.0);
        }
        Response::Fault(fault) => {
            e.byte(5);
            e.byte(match fault {
                StoreFault::StaleFence => 1,
                StoreFault::GenerationConflict => 2,
                StoreFault::AlreadyExists => 3,
                StoreFault::MissingPlayer => 4,
                StoreFault::RequestConflict => 5,
                StoreFault::Limit => 6,
            });
        }
    }
    Ok(e.finish())
}

pub(crate) fn decode_response(bytes: &[u8]) -> io::Result<Response> {
    let mut d = Decoder::new(bytes);
    if d.byte()? != 1 {
        return Err(invalid("unsupported response version"));
    }
    let response = match d.byte()? {
        0 => Response::Ok,
        1 => Response::Claimed(d.fence()?),
        2 => {
            let count = d.count()?;
            let mut entries = Vec::new();
            for _ in 0..count {
                entries.push(d.entry()?);
            }
            Response::Directory(entries)
        }
        3 => Response::Player(match d.byte()? {
            0 => None,
            1 => Some(d.record()?),
            _ => return Err(invalid("player flag")),
        }),
        4 => Response::Committed(Generation(d.u64()?)),
        5 => Response::Fault(match d.byte()? {
            1 => StoreFault::StaleFence,
            2 => StoreFault::GenerationConflict,
            3 => StoreFault::AlreadyExists,
            4 => StoreFault::MissingPlayer,
            5 => StoreFault::RequestConflict,
            6 => StoreFault::Limit,
            _ => return Err(invalid("unknown fault")),
        }),
        _ => return Err(invalid("unknown response")),
    };
    d.end()?;
    Ok(response)
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "frame deadline elapsed"))
}

fn read_exact(stream: &mut TcpStream, mut bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        match stream.read(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete frame",
                ));
            }
            Ok(n) => {
                bytes = &mut bytes[n..];
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn read_frame(stream: &mut TcpStream, timeout: Duration) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut header = [0; 4];
    read_exact(stream, &mut header, deadline)?;
    let count = u32::from_le_bytes(header) as usize;
    if count > MAX_FRAME {
        return Err(invalid("frame exceeds 1 MiB"));
    }
    let mut bytes = vec![0; count];
    read_exact(stream, &mut bytes, deadline)?;
    Ok(bytes)
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8], timeout: Duration) -> io::Result<()> {
    if bytes.len() > MAX_FRAME {
        return Err(invalid("frame exceeds 1 MiB"));
    }
    let deadline = Instant::now() + timeout;
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend((bytes.len() as u32).to_le_bytes());
    frame.extend(bytes);
    let mut rest = frame.as_slice();
    while !rest.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        match stream.write(rest) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "incomplete frame")),
            Ok(n) => rest = &rest[n..],
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn read_request(stream: &mut TcpStream, timeout: Duration) -> io::Result<Command> {
    decode_command(&read_frame(stream, timeout)?)
}
pub fn read_response(stream: &mut TcpStream, timeout: Duration) -> io::Result<Response> {
    decode_response(&read_frame(stream, timeout)?)
}
pub fn write_request(
    stream: &mut TcpStream,
    command: &Command,
    timeout: Duration,
) -> io::Result<()> {
    write_frame(stream, &encode_command(command)?, timeout)
}
pub fn write_response(
    stream: &mut TcpStream,
    response: &Response,
    timeout: Duration,
) -> io::Result<()> {
    write_frame(stream, &encode_response(response)?, timeout)
}
