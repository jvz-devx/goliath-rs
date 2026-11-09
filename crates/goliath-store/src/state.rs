//! Owns directory leases, opaque player records and durable retry receipts.

use crate::codec::{Decoder, Encoder};
use crate::receipts::{CAPACITY, Receipts};
use goliath_model::{
    Epoch, Fence, Generation, LeaseGrant, OwnershipCas, PlayerId, ProcessId, Record, StoreError,
    StoreLease, TileId, accepts_fence,
};
use std::collections::BTreeMap;
use std::time::Instant;

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
    /// Live updates need a newer wall anchor, or the identical existing clock.
    /// This ordering never grants or extends monotonic lease authority.
    SetClock {
        fence: Fence,
        clock: ClusterClock,
    },
    /// New tiles use this instead of the legacy, unleased Claim.
    ClaimLease {
        tile: TileId,
        lease_ms: u64,
    },
    RenewLease {
        fence: Fence,
        counter: u64,
    },
    /// Stop renewals, then retry ClaimLease after the last granted window ends.
    BeginTakeover(Fence),
    /// Activation check only. It does not renew or grant a local deadline.
    CheckLease(Fence),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreFault {
    StaleFence,
    GenerationConflict,
    AlreadyExists,
    MissingPlayer,
    RequestConflict,
    Limit,
    LeaseBusy,
    LeaseExpired,
    LeaseRequired,
    RenewalReplay,
    LeaseDraining,
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
    Leased(LeaseGrant),
}

#[derive(Default)]
pub(crate) struct State {
    next_process: u64,
    tiles: BTreeMap<TileId, DirectoryEntry>,
    players: BTreeMap<PlayerId, Record<Blob>>,
    receipts: Receipts,
    uuid_players: BTreeMap<PlayerKey, Record<Blob>>,
    clock: Option<ClusterClock>,
    leases: BTreeMap<TileId, StoreLease>,
    // The last renewal answered without a journal record, per tile, so its
    // transport retry gets the same grant. Never serialized, and kept out of
    // `receipts` so journal replay sees the same retry window as the live run.
    unjournaled: BTreeMap<TileId, (Command, Response)>,
    // Set once the journal has failed. Nothing is answered early after that.
    closed: bool,
    // None only during deterministic historical journal replay.
    now: Option<Instant>,
    #[cfg(feature = "fault-injection")]
    pub(crate) disable_fence: bool,
}

pub(crate) enum Change {
    Tile(DirectoryEntry),
    Player(PlayerId, Record<Blob>),
    Uuid(PlayerKey, Record<Blob>),
    Clock(ClusterClock),
    Lease(DirectoryEntry, StoreLease),
}

#[cfg(test)]
mod tests;

impl State {
    fn current(&self, tile: TileId) -> Option<Fence> {
        self.tiles
            .get(&tile)
            .filter(|e| e.active && self.live(tile))
            .map(|e| e.fence)
    }

    fn live(&self, tile: TileId) -> bool {
        self.now
            .is_none_or(|now| self.leases.get(&tile).is_none_or(|lease| lease.active(now)))
    }

    fn check(&self, supplied: Fence) -> Result<(), StoreFault> {
        if accepts_fence(self.current(supplied.tile), supplied) {
            Ok(())
        } else if self
            .tiles
            .get(&supplied.tile)
            .is_some_and(|e| e.active && e.fence == supplied)
            && !self.live(supplied.tile)
        {
            Err(StoreFault::LeaseExpired)
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
            Operation::RenewLease { fence, .. }
            | Operation::CheckLease(fence)
            | Operation::BeginTakeover(fence) => {
                self.check(*fence)?;
                let lease = self
                    .leases
                    .get(&fence.tile)
                    .ok_or(StoreFault::LeaseRequired)?;
                if matches!(command.operation, Operation::RenewLease { .. }) && lease.draining() {
                    return Err(StoreFault::LeaseDraining);
                }
            }
            Operation::Claim(tile) if self.leases.contains_key(tile) => {
                return Err(StoreFault::LeaseRequired);
            }
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
            Operation::Heartbeat { fence, .. } => {
                self.check(*fence)?;
                if self.leases.contains_key(&fence.tile) {
                    return Err(StoreFault::LeaseRequired);
                }
            }
            Operation::Release(fence) if self.leases.contains_key(&fence.tile) => {
                self.check(*fence)?
            }
            Operation::Release(fence)
                if !accepts_fence(self.tiles.get(&fence.tile).map(|e| e.fence), *fence) =>
            {
                return Err(StoreFault::StaleFence);
            }
            _ => {}
        }
        if let Some((previous, response)) = self.receipts.get(&command.id) {
            if let Response::Leased(grant) = response {
                self.check(grant.fence)?;
            }
            return if previous == command {
                Ok((response.clone(), None))
            } else {
                Err(StoreFault::RequestConflict)
            };
        }
        let (response, change) = match &command.operation {
            Operation::CheckLease(_) => return Ok((Response::Ok, None)),
            Operation::BeginTakeover(fence) => {
                let entry = self
                    .tiles
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::StaleFence)?;
                let mut lease = self
                    .leases
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::LeaseRequired)?;
                lease.drain();
                (Response::Ok, Change::Lease(entry, lease))
            }
            Operation::RenewLease { fence, counter } => {
                let mut lease = self
                    .leases
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::LeaseRequired)?;
                if *counter <= lease.counter() {
                    return Err(StoreFault::RenewalReplay);
                }
                let now = self.now.unwrap_or_else(Instant::now);
                if self.now.is_none() {
                    lease = StoreLease::recover(lease.lease_ms(), *counter, lease.draining(), now)
                        .ok_or(StoreFault::Limit)?;
                } else if !lease.renew(*counter, now) {
                    return Err(StoreFault::LeaseExpired);
                }
                let mut entry = self
                    .tiles
                    .get(&fence.tile)
                    .cloned()
                    .ok_or(StoreFault::StaleFence)?;
                entry.heartbeat = *counter;
                (
                    Response::Leased(lease.grant(*fence)),
                    Change::Lease(entry, lease),
                )
            }
            Operation::ClaimLease { tile, lease_ms } => {
                let now = self.now.unwrap_or_else(Instant::now);
                let lease = StoreLease::new(*lease_ms, now).ok_or(StoreFault::Limit)?;
                if self.now.is_some()
                    && (self.leases.get(tile).is_some_and(|old| old.active(now))
                        || (!self.leases.contains_key(tile)
                            && self.tiles.get(tile).is_some_and(|entry| entry.active)))
                {
                    return Err(StoreFault::LeaseBusy);
                }
                let entry = self.next_entry(*tile)?;
                (
                    Response::Leased(lease.grant(entry.fence)),
                    Change::Lease(entry, lease),
                )
            }
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
            Operation::SetClock { clock, .. } => {
                if self.now.is_some()
                    && self.clock.as_ref().is_some_and(|current| {
                        clock.unix_ms < current.unix_ms
                            || (clock.unix_ms == current.unix_ms && clock != current)
                    })
                {
                    return Err(StoreFault::RequestConflict);
                }
                // The wall anchor orders sky updates only. Lease authorization
                // uses separate monotonic Instants. Historical replay preserves
                // already-accepted clock writes instead of re-admitting them.
                (Response::Ok, Change::Clock(clock.clone()))
            }
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
                    Response::Directory(
                        self.tiles
                            .values()
                            .cloned()
                            .map(|mut entry| {
                                entry.active &= self.live(entry.fence.tile);
                                entry
                            })
                            .collect(),
                    ),
                    None,
                ));
            }
            Operation::Read(player) => {
                return Ok((Response::Player(self.players.get(player).cloned()), None));
            }
            Operation::Compact => return Ok((Response::Ok, None)),
            Operation::Claim(tile) => {
                let entry = self.next_entry(*tile)?;
                (Response::Claimed(entry.fence), Change::Tile(entry))
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

    /// Answers a renewal that needs neither a journal record nor a barrier.
    /// `None` sends the command down the journaled path unchanged, which is
    /// where every refusal comes from.
    ///
    /// The caller holds the one lock that also covers `prepare` and `apply`,
    /// and reads `now` under it. The extension therefore lands in the lease a
    /// later takeover or claim reads, and a drain, release or successor that
    /// was applied first, durable or not, is seen here. Only a settled lease
    /// is extended: its claim is on disk, so a restart finds it and waits a
    /// full lease from a time after this grant (`restart_leases`).
    pub(crate) fn renew_unjournaled(
        &mut self,
        command: &Command,
        now: Instant,
    ) -> Option<Response> {
        let Operation::RenewLease { fence, counter } = command.operation else {
            return None;
        };
        if self.closed || command.id.0 == 0 {
            return None;
        }
        self.set_now(now);
        self.check(fence).ok()?;
        if let Some((previous, response)) = self.unjournaled.get(&fence.tile)
            && previous.id == command.id
        {
            // A retry never extends the deadline a second time.
            return Some(if previous == command {
                response.clone()
            } else {
                Response::Fault(StoreFault::RequestConflict)
            });
        }
        if self.receipts.get(&command.id).is_some() {
            return None;
        }
        let lease = self.leases.get_mut(&fence.tile)?;
        if !lease.renew_unjournaled(counter, now) {
            return None;
        }
        let response = Response::Leased(lease.grant(fence));
        if let Some(entry) = self.tiles.get_mut(&fence.tile) {
            entry.heartbeat = counter;
        }
        self.unjournaled
            .insert(fence.tile, (command.clone(), response.clone()));
        Some(response)
    }

    /// Every applied claim is now on stable storage.
    pub(crate) fn settle(&mut self) {
        for lease in self.leases.values_mut() {
            lease.settle();
        }
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
    }

    pub(crate) fn apply(&mut self, command: Command, response: Response, change: Change) {
        match change {
            Change::Lease(entry, lease) => {
                self.leases.insert(entry.fence.tile, lease);
                self.next_process = self.next_process.max(entry.fence.process.0);
                self.tiles.insert(entry.fence.tile, entry);
            }
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

    fn next_entry(&self, tile: TileId) -> Result<DirectoryEntry, StoreFault> {
        let epoch = self
            .tiles
            .get(&tile)
            .map_or(Some(1), |e| e.fence.epoch.0.checked_add(1))
            .ok_or(StoreFault::Limit)?;
        let process = self.next_process.checked_add(1).ok_or(StoreFault::Limit)?;
        Ok(DirectoryEntry {
            fence: Fence {
                tile,
                process: ProcessId(process),
                epoch: Epoch(epoch),
            },
            active: true,
            heartbeat: 0,
        })
    }

    pub(crate) fn set_now(&mut self, now: Instant) {
        self.now = Some(now);
    }

    pub(crate) fn restart_leases(&mut self) -> std::io::Result<()> {
        let now = Instant::now();
        for lease in self.leases.values_mut() {
            *lease = StoreLease::recover(lease.lease_ms(), lease.counter(), lease.draining(), now)
                .ok_or_else(|| crate::codec::invalid("invalid durable lease"))?;
        }
        self.set_now(now);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> std::io::Result<Vec<u8>> {
        self.snapshot_into(Vec::new())
    }

    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.len()
    }

    pub(crate) fn receipt_blob_bytes(&self) -> (usize, usize) {
        self.receipts.blob_bytes()
    }

    pub(crate) fn player_blob_stats(&self) -> (usize, usize, usize, usize) {
        let (mut bytes, mut capacity) = (0, 0);
        for record in self.players.values().chain(self.uuid_players.values()) {
            bytes += record.inventory.bytes.len();
            capacity += record.inventory.bytes.capacity();
        }
        (self.players.len(), self.uuid_players.len(), bytes, capacity)
    }

    pub(crate) fn snapshot_into(&self, buffer: Vec<u8>) -> std::io::Result<Vec<u8>> {
        let mut e = Encoder::reuse(buffer);
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
        e.count(self.leases.len())?;
        for (tile, lease) in &self.leases {
            e.u64(tile.0 as u64);
            e.u64(lease.lease_ms());
            e.u64(lease.counter());
            e.u64(u64::from(lease.draining()));
        }
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
        let receipt_count = d.count()?;
        if receipt_count > CAPACITY {
            return Err(crate::codec::invalid(
                "snapshot retry history exceeds the bounded receipt window; explicit migration required",
            ));
        }
        for _ in 0..receipt_count {
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
        if !d.is_empty() {
            for _ in 0..d.count()? {
                let tile = TileId(d.index()?);
                let lease_ms = d.u64()?;
                let counter = d.u64()?;
                let draining = match d.u64()? {
                    0 => false,
                    1 => true,
                    _ => return Err(crate::codec::invalid("invalid lease drain flag")),
                };
                if !state.tiles.contains_key(&tile) || state.leases.contains_key(&tile) {
                    return Err(crate::codec::invalid("invalid lease tile"));
                }
                let lease = StoreLease::recover(lease_ms, counter, draining, Instant::now())
                    .ok_or_else(|| crate::codec::invalid("invalid durable lease"))?;
                state.leases.insert(tile, lease);
            }
        }
        d.end()?;
        Ok(state)
    }
}
