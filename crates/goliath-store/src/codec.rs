//! Owns the versioned, size-bounded binary encoding used by TCP and the journal.

use crate::state::{
    Blob, ClusterClock, Command, DirectoryEntry, MAX_BLOB, Operation, PlayerKey, RequestId,
    Response, StoreFault,
};
use goliath_model::{
    Epoch, Fence, Generation, LeaseGrant, OwnershipCas, PlayerId, ProcessId, Record, TileId,
};
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
    pub(crate) fn reuse(mut bytes: Vec<u8>) -> Self {
        bytes.clear();
        Self(bytes)
    }
    pub(crate) fn uuid(&mut self, key: PlayerKey) {
        self.0.extend(key.0.to_le_bytes());
    }
    pub(crate) fn clock(&mut self, clock: Option<&ClusterClock>) {
        self.byte(u8::from(clock.is_some()));
        if let Some(clock) = clock {
            self.u64(clock.unix_ms);
            self.u64(clock.game_tick);
            self.u64(clock.day_tick);
            self.byte(u8::from(clock.raining));
            self.byte(u8::from(clock.thundering));
        }
    }
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
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn uuid(&mut self) -> io::Result<PlayerKey> {
        Ok(PlayerKey(u128::from_le_bytes(
            self.take(16)?.try_into().map_err(|_| invalid("uuid"))?,
        )))
    }
    pub(crate) fn clock(&mut self) -> io::Result<Option<ClusterClock>> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(ClusterClock {
                unix_ms: self.u64()?,
                game_tick: self.u64()?,
                day_tick: self.u64()?,
                raining: self.flag()?,
                thundering: self.flag()?,
            })),
            _ => Err(invalid("clock flag")),
        }
    }
    fn flag(&mut self) -> io::Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("boolean")),
        }
    }
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
        Operation::ReadUuid(player) => {
            e.byte(9);
            e.uuid(*player);
        }
        Operation::CreateUuid {
            player,
            owner,
            blob,
        } => {
            e.byte(10);
            e.uuid(*player);
            e.fence(*owner);
            e.blob(blob)?;
        }
        Operation::CommitUuid { player, cas, blob } => {
            e.byte(11);
            e.uuid(*player);
            e.u64(cas.generation.0);
            e.fence(cas.source);
            e.fence(cas.target);
            e.blob(blob)?;
        }
        Operation::Players { after } => {
            e.byte(12);
            e.byte(u8::from(after.is_some()));
            if let Some(key) = after {
                e.uuid(*key);
            }
        }
        Operation::Clock => e.byte(13),
        Operation::SetClock { fence, clock } => {
            e.byte(14);
            e.fence(*fence);
            e.clock(Some(clock));
        }
        Operation::ClaimLease { tile, lease_ms } => {
            e.byte(15);
            e.u64(tile.0 as u64);
            e.u64(*lease_ms);
        }
        Operation::RenewLease { fence, counter } => {
            e.byte(16);
            e.fence(*fence);
            e.u64(*counter);
        }
        Operation::BeginTakeover(fence) => {
            e.byte(17);
            e.fence(*fence);
        }
        Operation::CheckLease(fence) => {
            e.byte(18);
            e.fence(*fence);
        }
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
        9 => Operation::ReadUuid(d.uuid()?),
        10 => Operation::CreateUuid {
            player: d.uuid()?,
            owner: d.fence()?,
            blob: d.blob()?,
        },
        11 => Operation::CommitUuid {
            player: d.uuid()?,
            cas: OwnershipCas {
                generation: Generation(d.u64()?),
                source: d.fence()?,
                target: d.fence()?,
            },
            blob: d.blob()?,
        },
        12 => Operation::Players {
            after: if d.flag()? { Some(d.uuid()?) } else { None },
        },
        13 => Operation::Clock,
        14 => Operation::SetClock {
            fence: d.fence()?,
            clock: d.clock()?.ok_or_else(|| invalid("missing clock"))?,
        },
        15 => Operation::ClaimLease {
            tile: TileId(d.index()?),
            lease_ms: d.u64()?,
        },
        16 => Operation::RenewLease {
            fence: d.fence()?,
            counter: d.u64()?,
        },
        17 => Operation::BeginTakeover(d.fence()?),
        18 => Operation::CheckLease(d.fence()?),
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
                StoreFault::LeaseBusy => 7,
                StoreFault::LeaseExpired => 8,
                StoreFault::LeaseRequired => 9,
                StoreFault::RenewalReplay => 10,
                StoreFault::LeaseDraining => 11,
            });
        }
        Response::Players(keys) => {
            e.byte(6);
            e.count(keys.len())?;
            for key in keys {
                e.uuid(*key);
            }
        }
        Response::Clock(clock) => {
            e.byte(7);
            e.clock(clock.as_ref());
        }
        Response::Leased(grant) => {
            e.byte(8);
            e.fence(grant.fence);
            e.u64(grant.lease_ms);
            e.u64(grant.counter);
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
        6 => {
            let count = d.count()?;
            let mut keys = Vec::new();
            for _ in 0..count {
                keys.push(d.uuid()?);
            }
            Response::Players(keys)
        }
        7 => Response::Clock(d.clock()?),
        8 => Response::Leased(LeaseGrant {
            fence: d.fence()?,
            lease_ms: d.u64()?,
            counter: d.u64()?,
        }),
        5 => Response::Fault(match d.byte()? {
            1 => StoreFault::StaleFence,
            2 => StoreFault::GenerationConflict,
            3 => StoreFault::AlreadyExists,
            4 => StoreFault::MissingPlayer,
            5 => StoreFault::RequestConflict,
            6 => StoreFault::Limit,
            7 => StoreFault::LeaseBusy,
            8 => StoreFault::LeaseExpired,
            9 => StoreFault::LeaseRequired,
            10 => StoreFault::RenewalReplay,
            11 => StoreFault::LeaseDraining,
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
    read_frame_before(stream, deadline)
}

fn read_frame_before(stream: &mut TcpStream, deadline: Instant) -> io::Result<Vec<u8>> {
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
    write_frame_before(stream, bytes, Instant::now() + timeout)
}

fn write_frame_before(stream: &mut TcpStream, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    if bytes.len() > MAX_FRAME {
        return Err(invalid("frame exceeds 1 MiB"));
    }
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

pub(crate) fn read_response_before(
    stream: &mut TcpStream,
    deadline: Instant,
) -> io::Result<Response> {
    decode_response(&read_frame_before(stream, deadline)?)
}

pub(crate) fn write_request_before(
    stream: &mut TcpStream,
    command: &Command,
    deadline: Instant,
) -> io::Result<()> {
    write_frame_before(stream, &encode_command(command)?, deadline)
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
