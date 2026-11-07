//! Owns directory leases, opaque player records and durable retry receipts.

use crate::codec::{Decoder, Encoder};
use goliath_model::{
    Epoch, Fence, Generation, OwnershipCas, PlayerId, ProcessId, Record, StoreError, TileId,
    accepts_fence,
};
use std::collections::BTreeMap;

pub(crate) const MAX_BLOB: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequestId(pub u128);

/// Full Minecraft UUID. Separate from the simulator's small integer identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PlayerKey(pub u128);

/// One durable cluster sky. Ticks advance from this wall-clock anchor, not tile ticks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterClock {
    pub unix_ms: u64,
    pub game_tick: u64,
    pub day_tick: u64,
    pub raining: bool,
    pub thundering: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blob {
    pub version: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub fence: Fence,
    pub active: bool,
    /// Client-supplied monotonically increasing liveness counter, not a lease.
    pub heartbeat: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    /// Globally unique, caller-owned, retained across all retries.
    pub id: RequestId,
    pub operation: Operation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Claim(TileId),
    Directory,
    Read(PlayerId),
    Create {
        player: PlayerId,
        owner: Fence,
        blob: Blob,
    },
    Commit {
        player: PlayerId,
        cas: OwnershipCas,
        blob: Blob,
    },
    Release(Fence),
    Heartbeat {
        fence: Fence,
        counter: u64,
    },
    Compact,
    ReadUuid(PlayerKey),
    CreateUuid {
        player: PlayerKey,
        owner: Fence,
        blob: Blob,
    },
    CommitUuid {
        player: PlayerKey,
        cas: OwnershipCas,
        blob: Blob,
    },
    /// Bounded UUID listing for startup reconciliation. Fetch blobs separately.
    Players {
        after: Option<PlayerKey>,
    },
    Clock,
    SetClock {
        fence: Fence,
        clock: ClusterClock,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreFault {
    StaleFence,
    GenerationConflict,
    AlreadyExists,
    MissingPlayer,
    RequestConflict,
    Limit,
}

impl From<StoreError> for StoreFault {
    fn from(value: StoreError) -> Self {
        match value {
            StoreError::StaleFence => Self::StaleFence,
            StoreError::GenerationConflict => Self::GenerationConflict,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    Ok,
    Claimed(Fence),
    Directory(Vec<DirectoryEntry>),
    Player(Option<Record<Blob>>),
    Committed(Generation),
    Fault(StoreFault),
    Players(Vec<PlayerKey>),
    Clock(Option<ClusterClock>),
}

#[derive(Default)]
pub(crate) struct State {
    next_process: u64,
    tiles: BTreeMap<TileId, DirectoryEntry>,
    players: BTreeMap<PlayerId, Record<Blob>>,
    receipts: BTreeMap<RequestId, (Command, Response)>,
    uuid_players: BTreeMap<PlayerKey, Record<Blob>>,
    clock: Option<ClusterClock>,
    #[cfg(feature = "fault-injection")]
    pub(crate) disable_fence: bool,
}

pub(crate) enum Change {
    Tile(DirectoryEntry),
    Player(PlayerId, Record<Blob>),
    Uuid(PlayerKey, Record<Blob>),
    Clock(ClusterClock),
}

impl State {
    fn current(&self, tile: TileId) -> Option<Fence> {
        self.tiles.get(&tile).filter(|e| e.active).map(|e| e.fence)
    }

    fn check(&self, supplied: Fence) -> Result<(), StoreFault> {
        if accepts_fence(self.current(supplied.tile), supplied) {
            Ok(())
        } else {
            Err(StoreFault::StaleFence)
        }
    }

    fn fences(&self, cas: OwnershipCas) -> (Option<Fence>, Option<Fence>) {
        #[cfg(feature = "fault-injection")]
        if self.disable_fence {
            return (Some(cas.source), Some(cas.target));
        }
        (self.current(cas.source.tile), self.current(cas.target.tile))
    }

    pub(crate) fn prepare(
        &self,
        command: &Command,
    ) -> Result<(Response, Option<Change>), StoreFault> {
        // Authorization precedes retry lookup. A receipt never renews authority.
        match &command.operation {
            Operation::Commit { cas, .. } | Operation::CommitUuid { cas, .. } => {
                let (source, target) = self.fences(*cas);
                cas.check_fences(source, target)?;
            }
            Operation::Create { owner, .. } => self.check(*owner)?,
            Operation::CreateUuid { owner, .. } => self.check(*owner)?,
            Operation::SetClock { fence, .. } => {
                self.check(*fence)?;
                if fence.tile != TileId(0) {
                    return Err(StoreFault::StaleFence);
                }
            }
            Operation::Heartbeat { fence, .. } => self.check(*fence)?,
            Operation::Release(fence)
                if !accepts_fence(self.tiles.get(&fence.tile).map(|e| e.fence), *fence) =>
            {
                return Err(StoreFault::StaleFence);
            }
            _ => {}
        }
        if let Some((previous, response)) = self.receipts.get(&command.id) {
            return if previous == command {
                Ok((response.clone(), None))
            } else {
                Err(StoreFault::RequestConflict)
            };
        }
        let (response, change) = match &command.operation {
            Operation::ReadUuid(player) => {
                return Ok((
                    Response::Player(self.uuid_players.get(player).cloned()),
                    None,
                ));
            }
            Operation::Players { after } => {
                return Ok((
                    Response::Players(
                        self.uuid_players
                            .keys()
                            .filter(|key| after.is_none_or(|after| **key > after))
                            .take(256)
                            .copied()
                            .collect(),
                    ),
                    None,
                ));
            }
            Operation::Clock => return Ok((Response::Clock(self.clock.clone()), None)),
            Operation::SetClock { clock, .. } => (Response::Ok, Change::Clock(clock.clone())),
            Operation::CreateUuid {
                player,
                owner,
                blob,
            } => {
                if self.uuid_players.contains_key(player) {
                    return Err(StoreFault::AlreadyExists);
                }
                (
                    Response::Committed(Generation(0)),
                    Change::Uuid(
                        *player,
                        Record {
                            generation: Generation(0),
                            owner: owner.tile,
                            inventory: blob.clone(),
                        },
                    ),
                )
            }
            Operation::CommitUuid { player, cas, blob } => {
                let mut record = self
                    .uuid_players
                    .get(player)
                    .cloned()
                    .ok_or(StoreFault::MissingPlayer)?;
                let (source, target) = self.fences(*cas);
                cas.commit(&mut record, blob, source, target)?;
                (
                    Response::Committed(record.generation),
                    Change::Uuid(*player, record),
                )
            }
            Operation::Directory => {
                return Ok((
                    Response::Directory(self.tiles.values().cloned().collect()),
                    None,
                ));
            }
            Operation::Read(player) => {
                return Ok((Response::Player(self.players.get(player).cloned()), None));
            }
            Operation::Compact => return Ok((Response::Ok, None)),
            Operation::Claim(tile) => {
                let epoch = self
                    .tiles
                    .get(tile)
                    .map_or(Some(1), |e| e.fence.epoch.0.checked_add(1))
                    .ok_or(StoreFault::Limit)?;
                let process = self.next_process.checked_add(1).ok_or(StoreFault::Limit)?;
                let fence = Fence {
                    tile: *tile,
                    process: ProcessId(process),
                    epoch: Epoch(epoch),
                };
                (
                    Response::Claimed(fence),
                    Change::Tile(DirectoryEntry {
                        fence,
                        active: true,
                        heartbeat: 0,
                    }),
                )
            }
            Operation::Create {
                player,
                owner,
                blob,
            } => {
                if self.players.contains_key(player) {
                    return Err(StoreFault::AlreadyExists);
                }
                let record = Record {
                    generation: Generation(0),
                    owner: owner.tile,
                    inventory: blob.clone(),
                };
                (
                    Response::Committed(Generation(0)),
                    Change::Player(*player, record),
                )
            }
            Operation::Commit { player, cas, blob } => {
                let mut record = self
                    .players
                    .get(player)
                    .cloned()
                    .ok_or(StoreFault::MissingPlayer)?;
                let (source, target) = self.fences(*cas);
                cas.commit(&mut record, blob, source, target)?;
                (
                    Response::Committed(record.generation),
                    Change::Player(*player, record),
                )
            }
            Operation::Release(fence) => {
                let mut entry = self
                    .tiles
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::StaleFence)?;
                entry.active = false;
                (Response::Ok, Change::Tile(entry))
            }
            Operation::Heartbeat { fence, counter } => {
                let mut entry = self
                    .tiles
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::StaleFence)?;
                entry.heartbeat = entry.heartbeat.max(*counter);
                (Response::Ok, Change::Tile(entry))
            }
        };
        if command.id.0 == 0 {
            return Err(StoreFault::RequestConflict);
        }
        match &command.operation {
            Operation::Create { blob, .. }
            | Operation::Commit { blob, .. }
            | Operation::CreateUuid { blob, .. }
            | Operation::CommitUuid { blob, .. }
                if blob.bytes.len() > MAX_BLOB =>
            {
                return Err(StoreFault::Limit);
            }
            _ => {}
        }
        Ok((response, Some(change)))
    }

    pub(crate) fn apply(&mut self, command: Command, response: Response, change: Change) {
        match change {
            Change::Tile(entry) => {
                self.next_process = self.next_process.max(entry.fence.process.0);
                self.tiles.insert(entry.fence.tile, entry);
            }
            Change::Player(player, record) => {
                self.players.insert(player, record);
            }
            Change::Uuid(player, record) => {
                self.uuid_players.insert(player, record);
            }
            Change::Clock(clock) => {
                self.clock = Some(clock);
            }
        }
        self.receipts.insert(command.id, (command, response));
    }

    pub(crate) fn snapshot(&self) -> std::io::Result<Vec<u8>> {
        let mut e = Encoder::default();
        e.u64(self.next_process);
        e.count(self.tiles.len())?;
        for entry in self.tiles.values() {
            e.entry(entry);
        }
        e.count(self.players.len())?;
        for (player, record) in &self.players {
            e.u64(player.0 as u64);
            e.record(record)?;
        }
        e.count(self.receipts.len())?;
        for (command, response) in self.receipts.values() {
            e.bytes(&crate::codec::encode_command(command)?)?;
            e.bytes(&crate::codec::encode_response(response)?)?;
        }
        e.count(self.uuid_players.len())?;
        for (player, record) in &self.uuid_players {
            e.uuid(*player);
            e.record(record)?;
        }
        e.clock(self.clock.as_ref());
        Ok(e.finish())
    }

    pub(crate) fn restore(bytes: &[u8]) -> std::io::Result<Self> {
        let mut d = Decoder::new(bytes);
        let mut state = Self {
            next_process: d.u64()?,
            ..Self::default()
        };
        for _ in 0..d.count()? {
            let entry = d.entry()?;
            state.tiles.insert(entry.fence.tile, entry);
        }
        for _ in 0..d.count()? {
            let player = PlayerId(d.index()?);
            state.players.insert(player, d.record()?);
        }
        for _ in 0..d.count()? {
            let command = crate::codec::decode_command(d.bytes()?)?;
            let response = crate::codec::decode_response(d.bytes()?)?;
            state.receipts.insert(command.id, (command, response));
        }
        // E5/E9 snapshots end after receipts. The extension preserves their format.
        if !d.is_empty() {
            for _ in 0..d.count()? {
                let player = d.uuid()?;
                state.uuid_players.insert(player, d.record()?);
            }
            state.clock = d.clock()?;
        }
        d.end()?;
        Ok(state)
    }
}
