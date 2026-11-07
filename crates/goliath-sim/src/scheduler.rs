//! Owns one virtual-clock queue, injected IO completions and measured outcomes.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::Write;

use goliath_model::{
    Checkpoint, Cluster, Cut, CutId, Delta, Epoch, Generation, Handover, ITEMS, Image, ItemId,
    PLAYERS, Place, PlayerId, Policy, Record, TILES, TileId,
};

use crate::scenario::{Action, Fault, FaultAt, Link, Scenario, mix};
use crate::{Mutation, Settings};

const END: u64 = 2_000;

#[derive(Clone, Debug)]
enum Event {
    Input(PlayerId, Action, ItemId, u64),
    Fault(Fault),
    CheckpointStart(TileId),
    CheckpointDone(Checkpoint, Option<usize>),
    Commit(usize, u64),
    Arrive(usize, u64),
    DurableArrive(usize),
    Recover(TileId, Epoch, u64),
    Release(usize, u64),
    Wal(TileId, Epoch, Vec<Delta>),
    Abort(usize),
    CutAppend(Cut),
    CutFlush(TileId, Epoch, u64),
    CutCompact(TileId, Epoch, CutId, Image),
}

impl Event {
    fn description(&self, at: u64) -> String {
        match self {
            Self::CheckpointDone(cp, flight) => format!(
                "{at:04} checkpoint durable tile={} epoch={} seq={} flight={flight:?}",
                cp.fence.tile.0, cp.fence.epoch.0, cp.sequence
            ),
            Self::Wal(tile, epoch, changes) => format!(
                "{at:04} WAL durable tile={} epoch={} {changes:?}",
                tile.0, epoch.0
            ),
            event => format!("{at:04} {event:?}"),
        }
    }
}

#[derive(Clone, Debug)]
struct Scheduled {
    at: u64,
    order: u64,
    event: Event,
}

impl PartialEq for Scheduled {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.order) == (other.at, other.order)
    }
}
impl Eq for Scheduled {}
impl PartialOrd for Scheduled {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Scheduled {
    fn cmp(&self, other: &Self) -> Ordering {
        (other.at, other.order).cmp(&(self.at, self.order))
    }
}

#[derive(Clone, Debug)]
struct Flight {
    handover: Handover,
    start: u64,
    committed: bool,
    finished: bool,
    aborted: bool,
    cut: Option<CutId>,
}

#[derive(Clone, Debug)]
struct Branch {
    flight: usize,
    images: [Image; TILES],
    record: Record,
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub hash: u64,
    pub duplicates: u64,
    pub losses: u64,
    pub loss_recovery: u64,
    pub loss_other: u64,
    pub rollback: u64,
    pub owner_violations: u64,
    pub stale_writes: u64,
    pub gaps: Vec<u64>,
    pub completed: u64,
    pub aborted: u64,
    pub censored: u64,
    pub rejected_inputs: u64,
    pub embargo_crossings: u64,
    pub action_wait: Vec<u64>,
    pub first_failure: Option<String>,
    pub trace: Vec<String>,
}

impl Outcome {
    pub fn violated(&self) -> bool {
        self.duplicates + self.owner_violations + self.stale_writes > 0
    }
}

struct Simulator<'a> {
    scenario: &'a Scenario,
    cluster: Cluster,
    queue: BinaryHeap<Scheduled>,
    next_order: u64,
    now: u64,
    frozen: [Option<usize>; PLAYERS],
    wal_busy: [Option<u64>; TILES],
    recovering: [bool; TILES],
    flights: Vec<Flight>,
    branch: Option<Branch>,
    partition: [bool; 3],
    store_down: bool,
    chaos: bool,
    previous_counts: [u64; ITEMS],
    duplicate_seen: [bool; ITEMS],
    outcome: Outcome,
    verbose: bool,
    settings: Settings,
    cut_flushing: [bool; TILES],
}

pub fn run(scenario: &Scenario, policy: Policy, verbose: bool) -> Outcome {
    run_with_settings(scenario, policy, verbose, Settings::default())
}

pub fn run_with_settings(
    scenario: &Scenario,
    policy: Policy,
    verbose: bool,
    settings: Settings,
) -> Outcome {
    let mut sim = Simulator {
        scenario,
        cluster: Cluster::new(policy),
        queue: BinaryHeap::new(),
        next_order: 0,
        now: 0,
        frozen: [None; PLAYERS],
        wal_busy: [None; TILES],
        recovering: [false; TILES],
        flights: Vec::new(),
        branch: None,
        partition: [false; 3],
        store_down: false,
        chaos: false,
        previous_counts: [1; ITEMS],
        duplicate_seen: [false; ITEMS],
        outcome: Outcome {
            hash: 0xcbf29ce484222325,
            ..Outcome::default()
        },
        verbose,
        settings,
        cut_flushing: [false; TILES],
    };
    if settings.mutation == Mutation::UnfencedCas {
        sim.cluster.directory.fencing = false;
    }
    for &(at, player, action, item) in &scenario.inputs {
        sim.schedule(at, Event::Input(player, action, item, at));
    }
    for &FaultAt { at, fault } in &scenario.faults {
        sim.schedule(at, Event::Fault(fault));
    }
    // Bounded fault experiment, not a claim of availability under an endless partition.
    sim.schedule(800, Event::Fault(Fault::Heal));
    for t in 0..TILES {
        sim.schedule(
            scenario.latency.checkpoint_interval_ms,
            Event::CheckpointStart(TileId(t)),
        );
    }
    while let Some(scheduled) = sim.queue.pop() {
        if scheduled.at > END {
            break;
        }
        sim.now = scheduled.at;
        let description = scheduled.event.description(sim.now);
        let recovery = matches!(scheduled.event, Event::Fault(Fault::Crash(_)));
        sim.step(scheduled.event);
        sim.check(&description, recovery);
    }
    // Restart every process together. This also detects dormant checkpoint duplicates.
    sim.abort_branch();
    let before = sim.cluster.audit();
    for t in 0..TILES {
        sim.crash_tile(TileId(t));
        sim.cluster.reconcile(TileId(t));
        sim.mutate_recovery(TileId(t));
    }
    sim.outcome.rollback += before.difference(&sim.cluster.audit());
    sim.check("2000 final recovery cut", true);
    for flight in &sim.flights {
        if !flight.finished && !flight.aborted {
            sim.outcome.censored += 1;
        }
    }
    sim.outcome
}

impl Simulator<'_> {
    fn schedule(&mut self, at: u64, event: Event) {
        self.queue.push(Scheduled {
            at,
            order: self.next_order,
            event,
        });
        self.next_order += 1;
    }

    fn check(&mut self, description: &str, recovery: bool) {
        let audit = self.cluster.audit();
        for (item, &count) in audit.counts.iter().enumerate() {
            if count > 1 && !self.duplicate_seen[item] {
                self.duplicate_seen[item] = true;
                self.outcome.duplicates += 1;
            }
            if count == 0 && self.previous_counts[item] > 0 {
                self.outcome.losses += 1;
                if recovery {
                    self.outcome.loss_recovery += 1;
                } else {
                    self.outcome.loss_other += 1;
                }
            }
        }
        self.previous_counts = audit.counts;
        self.outcome.owner_violations = self
            .outcome
            .owner_violations
            .max(audit.player_owner_violations);
        self.outcome.stale_writes = audit.stale_writes;
        if (audit.violated() || audit.missing_identities > 0)
            && self.outcome.first_failure.is_none()
        {
            self.outcome.first_failure = Some(format!("{description}: {audit:?}"));
        }
        // Explicit stable hash, not RandomState or platform-dependent DefaultHasher.
        let mut bytes = Vec::with_capacity(256);
        bytes.extend(description.bytes());
        for tile in &self.cluster.tiles {
            for places in tile.live.items {
                bytes.extend(places);
            }
            for actor in tile.live.players {
                bytes.extend(actor.map_or(u64::MAX, |a| a.generation.0).to_le_bytes());
            }
        }
        for record in self.cluster.store.records {
            bytes.extend(record.generation.0.to_le_bytes());
            bytes.extend((record.owner.0 as u64).to_le_bytes());
            bytes.extend(record.inventory);
        }
        bytes.extend(audit.stale_writes.to_le_bytes());
        for byte in bytes {
            self.outcome.hash ^= u64::from(byte);
            self.outcome.hash = self.outcome.hash.wrapping_mul(0x100000001b3);
        }
        if self.verbose {
            let mut line = String::new();
            write!(
                &mut line,
                "{description} counts={:?} owners={} stale={}",
                audit.counts, audit.player_owner_violations, audit.stale_writes
            )
            .expect("format into String");
            self.outcome.trace.push(line);
        }
    }

    fn step(&mut self, event: Event) {
        match event {
            Event::Input(player, action, item, requested) => {
                self.input(player, action, item, requested)
            }
            Event::Fault(fault) => self.fault(fault),
            Event::CheckpointStart(tile) => {
                if self.branch.is_none() && !self.recovering[tile.0] {
                    self.checkpoint(tile, None);
                }
                let next = self.now + self.scenario.latency.checkpoint_interval_ms;
                if next < 800 {
                    self.schedule(next, Event::CheckpointStart(tile));
                }
            }
            Event::CheckpointDone(checkpoint, flight) => {
                let tile = checkpoint.fence.tile;
                if self.cluster.directory.accepts(checkpoint.fence)
                    && ((self.settings.mutation == Mutation::P2MissingInstall && flight.is_some())
                        || self.cluster.tiles[tile.0].install_checkpoint(
                            &checkpoint,
                            self.cluster.directory.current(tile).epoch,
                        ))
                    && let Some(id) = flight
                    && !self.flights[id].aborted
                {
                    self.schedule(
                        self.now + self.scenario.latency.network_ms,
                        Event::Commit(id, 0),
                    );
                }
            }
            Event::Commit(id, attempt) => self.commit(id, attempt),
            Event::Arrive(id, attempt) => self.arrive(id, attempt),
            Event::DurableArrive(id) => self.land(id),
            Event::Recover(tile, epoch, attempt) => {
                if self.cluster.directory.current(tile).epoch == epoch {
                    if self.blocked(Link::TileStore, tile.0, attempt) {
                        self.schedule(
                            self.now + self.retry_delay(tile.0, attempt),
                            Event::Recover(tile, epoch, attempt + 1),
                        );
                    } else {
                        self.cluster.reconcile(tile);
                        self.mutate_recovery(tile);
                        self.recovering[tile.0] = false;
                    }
                }
            }
            Event::Release(id, attempt) => self.release(id, attempt),
            Event::Wal(tile, epoch, changes) => {
                if self.cluster.directory.current(tile).epoch == epoch {
                    self.cluster.tiles[tile.0].commit_delta(changes);
                    self.wal_busy[tile.0] = None;
                }
            }
            Event::Abort(id) => {
                if !self.flights[id].committed {
                    self.abort_flight(id);
                }
            }
            Event::CutAppend(cut) => {
                let tile = cut.fence.tile;
                let current = self.cluster.directory.current(tile);
                if self.cluster.tiles[tile.0].cut_log.append(
                    cut,
                    current,
                    self.settings.mutation == Mutation::P5StaleAppend,
                ) {
                    self.start_cut_flush(tile);
                }
            }
            Event::CutFlush(tile, epoch, end) => {
                if self.cluster.directory.current(tile).epoch == epoch {
                    let log = &mut self.cluster.tiles[tile.0].cut_log;
                    log.flush(end);
                    let (id, image) = log.compaction();
                    let newer = log.written() > end;
                    self.cut_flushing[tile.0] = false;
                    self.schedule(
                        self.now + self.scenario.latency.checkpoint_ms,
                        Event::CutCompact(tile, epoch, id, image),
                    );
                    for id in 0..self.flights.len() {
                        let flight = &self.flights[id];
                        if !flight.aborted
                            && !flight.committed
                            && flight.handover.source == tile
                            && flight.handover.source_fence.epoch == epoch
                            && flight
                                .cut
                                .is_some_and(|c| self.cluster.tiles[tile.0].cut_log.covers(c))
                        {
                            self.schedule(
                                self.now + self.scenario.latency.network_ms,
                                Event::Commit(id, 0),
                            );
                        }
                    }
                    if newer {
                        self.start_cut_flush(tile);
                    }
                }
            }
            Event::CutCompact(tile, epoch, id, image) => {
                if self.cluster.directory.current(tile).epoch == epoch {
                    self.cluster.tiles[tile.0].cut_log.compact(id, image);
                }
            }
        }
    }

    fn input(&mut self, player: PlayerId, action: Action, item: ItemId, requested: u64) {
        if self.frozen[player.0].is_some() {
            self.outcome.rejected_inputs += 1;
            return;
        }
        let Some(tile) = self.cluster.actor_tile(player) else {
            return;
        };
        if self.recovering[tile.0] {
            self.outcome.rejected_inputs += 1;
            return;
        }
        if let Some(until) = self.wal_busy[tile.0] {
            self.schedule(until, Event::Input(player, action, item, requested));
            return;
        }
        if let Action::Cross(target) = action {
            if self.recovering[target.0] {
                self.outcome.rejected_inputs += 1;
                return;
            }
            if self.branch.is_some() {
                self.outcome.embargo_crossings += 1;
                return;
            } // Speculation cannot escape its rollback domain.
            let Some(handover) = self.cluster.handover(player, target) else {
                return;
            };
            let id = self.flights.len();
            self.flights.push(Flight {
                handover,
                start: requested,
                committed: false,
                finished: false,
                aborted: false,
                cut: None,
            });
            self.frozen[player.0] = Some(id);
            match self.cluster.policy {
                Policy::P2 => self.checkpoint(tile, Some(id)),
                Policy::P5 => self.checkpoint(tile, Some(id)),
                Policy::P3 => {
                    self.branch = Some(Branch {
                        flight: id,
                        images: self.cluster.tiles.each_ref().map(|tile| tile.live),
                        record: self.cluster.store.records[player.0],
                    });
                    self.checkpoint(tile, Some(id));
                    self.schedule(
                        self.now + self.scenario.latency.network_ms,
                        Event::Arrive(id, 0),
                    );
                }
                _ => self.schedule(
                    self.now + self.scenario.latency.network_ms,
                    Event::Commit(id, 0),
                ),
            }
            return;
        }
        let mut image = self.cluster.tiles[tile.0].live;
        let expected = match action {
            Action::Take => Place::Chest,
            Action::PickUp => Place::Ground,
            Action::Drop | Action::Store => Place::Inventory(player),
            Action::Cross(_) => unreachable!("handled above"),
        };
        if image.items[item.0][expected.index()] == 0 {
            return;
        }
        image.items[item.0][expected.index()] -= 1;
        let destination = match action {
            Action::Take | Action::PickUp => Place::Inventory(player),
            Action::Drop => Place::Ground,
            Action::Store => Place::Chest,
            Action::Cross(_) => unreachable!("handled above"),
        };
        image.items[item.0][destination.index()] += 1;
        if self.cluster.policy == Policy::P4 {
            let delay = 50 - self.now % 50 + self.scenario.latency.fsync_ms;
            self.outcome.action_wait.push(self.now + delay - requested);
            self.wal_busy[tile.0] = Some(self.now + delay);
            self.schedule(
                self.now + delay,
                Event::Wal(
                    tile,
                    self.cluster.directory.current(tile).epoch,
                    Delta::between(self.cluster.tiles[tile.0].live, image),
                ),
            );
        } else {
            self.cluster.tiles[tile.0].change(image);
        }
    }

    fn checkpoint(&mut self, tile: TileId, flight: Option<usize>) {
        let fence = self.cluster.directory.current(tile);
        if self.cluster.policy == Policy::P5 {
            let image = self.cluster.tiles[tile.0].live;
            let cut = self.cluster.tiles[tile.0].cut_log.capture(
                image,
                fence,
                self.settings.mutation == Mutation::P5SkipChest,
            );
            if let Some(id) = flight {
                self.flights[id].cut = Some(cut.id);
                if self.settings.mutation == Mutation::P5EarlyCommit {
                    self.schedule(
                        self.now + self.scenario.latency.network_ms,
                        Event::Commit(id, 0),
                    );
                }
            }
            // Workers can complete captures in reverse order. The workload and
            // fault schedule stay the same as E1. This is IO variance, not input tuning.
            let delay = 1 + mix(self.scenario.seed ^ cut.id.0 ^ (tile.0 as u64 * 719))
                % self.scenario.latency.checkpoint_ms.max(1);
            self.schedule(self.now + delay, Event::CutAppend(cut));
            return;
        }
        let mut checkpoint = if self.cluster.policy == Policy::P4 {
            self.cluster.tiles[tile.0].prepare_logged_checkpoint(fence)
        } else {
            self.cluster.tiles[tile.0].prepare_checkpoint(fence)
        };
        if self.settings.mutation == Mutation::P2SplitSave {
            let old = self.cluster.tiles[tile.0]
                .prepare_logged_checkpoint(fence)
                .image;
            checkpoint.image.players = old.players;
            for (places, previous) in checkpoint.image.items.iter_mut().zip(old.items) {
                places[2..].copy_from_slice(&previous[2..]);
            }
        }
        if self.settings.mutation == Mutation::P2EarlyCommit
            && let Some(id) = flight
        {
            self.schedule(
                self.now + self.scenario.latency.network_ms,
                Event::Commit(id, 0),
            );
        }
        let delay = self.scenario.latency.checkpoint_ms + self.scenario.latency.fsync_ms;
        self.schedule(self.now + delay, Event::CheckpointDone(checkpoint, flight));
    }

    fn start_cut_flush(&mut self, tile: TileId) {
        if !self.cut_flushing[tile.0] {
            self.cut_flushing[tile.0] = true;
            self.schedule(
                self.now + self.scenario.latency.fsync_ms,
                Event::CutFlush(
                    tile,
                    self.cluster.directory.current(tile).epoch,
                    self.cluster.tiles[tile.0].cut_log.written(),
                ),
            );
        }
    }

    fn crash_tile(&mut self, tile: TileId) {
        let mut bad_image = None;
        if self.cluster.policy == Policy::P5 {
            self.cluster.tiles[tile.0].cut_log.tear_unflushed_tail();
            if self.settings.mutation == Mutation::P5ReplayPastTorn {
                bad_image = Some(self.cluster.tiles[tile.0].cut_log.recover(true));
            }
        }
        self.cluster.crash(tile);
        if let Some(image) = bad_image {
            self.cluster.tiles[tile.0].live = image;
        }
    }

    fn mutate_recovery(&mut self, tile: TileId) {
        if self.settings.mutation == Mutation::OlderStoreRecovery {
            for (p, record) in self.cluster.store.records.iter().enumerate() {
                if record.owner == tile {
                    self.cluster.tiles[tile.0].live.insert_player(
                        PlayerId(p),
                        record.generation,
                        record.inventory,
                    );
                }
            }
        }
    }

    fn blocked(&self, link: Link, id: usize, attempt: u64) -> bool {
        let partition = match link {
            Link::TileStore => 0,
            Link::GatewayTile => 1,
            Link::TileTile => 2,
        };
        self.partition[partition]
            || (link == Link::TileStore && self.store_down)
            || (self.chaos
                && mix(self.scenario.seed ^ (id as u64 * 719) ^ (attempt * 997) ^ partition as u64)
                    .is_multiple_of(4))
    }

    fn retry_delay(&self, id: usize, attempt: u64) -> u64 {
        15 + mix(self.scenario.seed ^ id as u64 ^ attempt) % 23
    }

    fn commit(&mut self, id: usize, attempt: u64) {
        if self.flights[id].aborted || self.flights[id].committed {
            return;
        }
        if attempt == 0 {
            // Request handling and its durable commit are distinct crash points.
            self.schedule(
                self.now + self.scenario.latency.fsync_ms,
                Event::Commit(id, 1),
            );
            return;
        }
        if self.blocked(Link::TileStore, id, attempt) {
            self.schedule(
                self.now + self.retry_delay(id, attempt),
                Event::Commit(id, attempt + 1),
            );
            return;
        }
        let handover = self.flights[id].handover;
        if let Some(branch) = &self.branch {
            // The tentative record was only a routing view, never durable store state.
            self.cluster.store.records[handover.player.0] = branch.record;
        }
        let accepted = if self.cluster.policy == Policy::P0 {
            self.cluster.store.records[handover.player.0] = Record {
                generation: Generation(handover.generation.0 + 1),
                owner: handover.target,
                inventory: handover.inventory,
            };
            true
        } else {
            self.cluster
                .store
                .commit(&self.cluster.directory, &handover)
                .is_ok()
        };
        if !accepted {
            self.abort_flight(id);
            return;
        }
        self.flights[id].committed = true;
        if self.cluster.policy == Policy::P3 {
            self.branch = None;
            // A tentative route may not yet have arrived. Finish through the same path.
        }
        self.schedule(
            self.now + self.scenario.latency.network_ms,
            Event::Arrive(id, 0),
        );
        if self.chaos {
            self.schedule(self.now + 1, Event::Commit(id, attempt + 1));
            self.schedule(self.now + 27, Event::Arrive(id, 0));
        }
    }

    fn arrive(&mut self, id: usize, attempt: u64) {
        if self.flights[id].aborted {
            return;
        }
        if self.blocked(Link::TileTile, id, attempt) || self.blocked(Link::GatewayTile, id, attempt)
        {
            self.schedule(
                self.now + self.retry_delay(id, attempt),
                Event::Arrive(id, attempt + 1),
            );
            return;
        }
        if self.cluster.policy == Policy::P4 {
            self.schedule(
                self.now + self.scenario.latency.fsync_ms,
                Event::DurableArrive(id),
            );
        } else {
            self.land(id);
        }
    }

    fn land(&mut self, id: usize) {
        if self.flights[id].aborted {
            return;
        }
        let handover = self.flights[id].handover;
        if self.recovering[handover.target.0] {
            self.schedule(self.now + 15, Event::Arrive(id, 0));
            return;
        }
        let tentative = self
            .branch
            .as_ref()
            .is_some_and(|branch| branch.flight == id);
        let record = self.cluster.store.records[handover.player.0];
        if !tentative
            && (record.owner != handover.target
                || record.generation != Generation(handover.generation.0 + 1))
        {
            return;
        }
        if tentative {
            self.cluster.store.records[handover.player.0] = Record {
                generation: Generation(handover.generation.0 + 1),
                owner: handover.target,
                inventory: handover.inventory,
            };
        }
        // A restarted target already imported the durable entry during recovery.
        if self.cluster.directory.current(handover.target) == handover.target_fence {
            self.cluster.arrive(handover);
            if self.cluster.policy == Policy::P0 {
                self.cluster.tiles[handover.target.0].baseline_save_player(
                    handover.player,
                    Generation(handover.generation.0 + 1),
                    handover.inventory,
                );
            }
        }
        if !self.flights[id].finished {
            self.outcome.gaps.push(self.now - self.flights[id].start);
            self.outcome.completed += 1;
            self.flights[id].finished = true;
            self.frozen[handover.player.0] = None;
        }
        if tentative {
            self.cluster.release(handover);
        } else {
            self.schedule(
                self.now + self.scenario.latency.network_ms,
                Event::Release(id, 0),
            );
        }
    }

    fn release(&mut self, id: usize, attempt: u64) {
        if self.flights[id].aborted {
            return;
        }
        if self.blocked(Link::GatewayTile, id, attempt) {
            self.schedule(
                self.now + self.retry_delay(id, attempt),
                Event::Release(id, attempt + 1),
            );
            return;
        }
        let handover = self.flights[id].handover;
        if self.cluster.directory.current(handover.source) == handover.source_fence {
            self.cluster.release(handover);
        }
    }

    fn abort_branch(&mut self) {
        let Some(branch) = self.branch.take() else {
            return;
        };
        let before = self.cluster.audit();
        for (tile, image) in self.cluster.tiles.iter_mut().zip(branch.images) {
            tile.change(image);
        }
        let player = self.flights[branch.flight].handover.player;
        self.cluster.store.records[player.0] = branch.record;
        self.outcome.rollback += before.difference(&self.cluster.audit());
        self.flights[branch.flight].aborted = true;
        self.outcome.aborted += 1;
        if !self.flights[branch.flight].finished {
            self.outcome
                .gaps
                .push(self.now - self.flights[branch.flight].start);
        }
        self.frozen[player.0] = None;
    }

    fn abort_flight(&mut self, id: usize) {
        if self
            .branch
            .as_ref()
            .is_some_and(|branch| branch.flight == id)
        {
            self.abort_branch();
            return;
        }
        if !self.flights[id].aborted {
            self.flights[id].aborted = true;
            self.outcome.aborted += 1;
            self.frozen[self.flights[id].handover.player.0] = None;
            if !self.flights[id].finished {
                self.outcome.gaps.push(self.now - self.flights[id].start);
            }
        }
    }

    fn fault(&mut self, fault: Fault) {
        match fault {
            Fault::Crash(tile) => {
                if self.settings.abort_delay_ms == 0 {
                    self.abort_branch();
                } else if let Some(branch) = &self.branch {
                    self.schedule(
                        self.now + self.settings.abort_delay_ms,
                        Event::Abort(branch.flight),
                    );
                }
                let before = self.cluster.audit();
                self.crash_tile(tile);
                self.mutate_recovery(tile);
                self.recovering[tile.0] = true;
                self.schedule(
                    self.now + self.scenario.latency.network_ms,
                    Event::Recover(tile, self.cluster.directory.current(tile).epoch, 0),
                );
                self.outcome.rollback += before.difference(&self.cluster.audit());
                self.wal_busy[tile.0] = None;
                self.cut_flushing[tile.0] = false;
                for id in 0..self.flights.len() {
                    let f = &self.flights[id];
                    if !f.committed
                        && !f.aborted
                        && (f.handover.source == tile || f.handover.target == tile)
                    {
                        if self.settings.abort_delay_ms == 0 {
                            self.abort_flight(id);
                        } else {
                            self.schedule(
                                self.now + self.settings.abort_delay_ms,
                                Event::Abort(id),
                            );
                        }
                    }
                }
            }
            Fault::Partition(link) => {
                self.partition[match link {
                    Link::TileStore => 0,
                    Link::GatewayTile => 1,
                    Link::TileTile => 2,
                }] = true;
            }
            Fault::StoreDown => self.store_down = true,
            Fault::NetworkChaos => self.chaos = true,
            Fault::Heal => {
                self.partition = [false; 3];
                self.store_down = false;
                self.chaos = false;
            }
        }
    }
}

/// Delete faults, then workload inputs, until no single deletion preserves failure.
/// This is 1-minimal under deletion, not a claim of globally shortest causality.
pub fn shrink(scenario: &Scenario, policy: Policy) -> Scenario {
    let mut reduced = scenario.clone();
    let original = run(scenario, policy, false);
    let fails = |candidate: &Scenario| {
        let result = run(candidate, policy, false);
        if original.duplicates > 0 {
            result.duplicates > 0
        } else if original.owner_violations > 0 {
            result.owner_violations > 0
        } else if original.stale_writes > 0 {
            result.stale_writes > 0
        } else {
            result.losses > 0
        }
    };
    if !fails(&reduced) {
        return reduced;
    }
    loop {
        let mut changed = false;
        for i in (0..reduced.faults.len()).rev() {
            let mut candidate = reduced.clone();
            candidate.faults.remove(i);
            if fails(&candidate) {
                reduced = candidate;
                changed = true;
            }
        }
        for i in (0..reduced.inputs.len()).rev() {
            let mut candidate = reduced.clone();
            candidate.inputs.remove(i);
            if fails(&candidate) {
                reduced = candidate;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    reduced
}
