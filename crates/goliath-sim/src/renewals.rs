//! Owns the store-crash renewal schedule and its independent authority checker.

use crate::scenario::mix;
use goliath_model::{
    Epoch, Fence, LeaseGrant, LeaseKeeper, LeaseRequest, ProcessId, RenewalAction, RenewalSchedule,
    StoreLease, TileId,
};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

const MS: u64 = 1_000_000;
const PROCESSES: usize = 3;

/// How the store answers a granted renewal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenewalPath {
    /// Journaled, answered after its batch's durable barrier.
    Barrier,
    /// Not journaled, answered at once from the live lease.
    Unjournaled,
}

/// Deliberate bugs. Each must be caught by the checker.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenewalFault {
    #[default]
    None,
    /// A restarted store treats recovered leases as already expired.
    NoRestartWait,
    /// A restarted store waits half a lease instead of a full one.
    ShortRestartWait,
    /// Early renewals are decided from the copy taken at the last barrier, so
    /// one is answered after a takeover moved the lease.
    StaleView,
    /// A takeover prepared before the barrier is applied after it and
    /// overwrites the extensions granted in between.
    LostExtension,
    /// Claims skip the barrier too.
    UnjournaledClaim,
    /// The tile ignores its local deadline.
    IgnoreDeadline,
}

impl RenewalFault {
    pub const PLANTED: [Self; 6] = [
        Self::NoRestartWait,
        Self::ShortRestartWait,
        Self::StaleView,
        Self::LostExtension,
        Self::UnjournaledClaim,
        Self::IgnoreDeadline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NoRestartWait => "no-restart-wait",
            Self::ShortRestartWait => "short-restart-wait",
            Self::StaleView => "stale-view",
            Self::LostExtension => "lost-extension",
            Self::UnjournaledClaim => "unjournaled-claim",
            Self::IgnoreDeadline => "ignores-deadline",
        }
    }
}

/// Durable-barrier duration profile, in real milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stall {
    /// 1 to 40 ms.
    Calm,
    /// 15% of barriers take 150 to 700 ms, as on the second test machine.
    Tail,
    /// Half of all barriers take 300 to 2,500 ms.
    Severe,
}

impl Stall {
    pub const ALL: [Self; 3] = [Self::Calm, Self::Tail, Self::Severe];

    pub fn name(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Tail => "tail",
            Self::Severe => "severe",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenewalSetup {
    pub lease_ms: u64,
    pub stall: Stall,
    pub path: RenewalPath,
    pub fault: RenewalFault,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenewalOutcome {
    /// Actions taken while another process would also act.
    pub overlapping_owners: u64,
    /// Actions by a fence older than one that already acted.
    pub stale_writes: u64,
    /// Actions the running store no longer counts as inside a live lease.
    pub beyond_store_window: u64,
    pub actions: u64,
    pub claims: u64,
    pub holds: u64,
    pub late_claims: u64,
    pub renewals: u64,
    pub unjournaled_grants: u64,
    pub store_crashes: u64,
    /// Crashes whose recovered counter was behind the one last granted.
    pub renewals_lost: u64,
    pub expiries: u64,
    /// Expiries of a connected, undrained holder that had renewed before, with
    /// the store up throughout. Only the store's queue and barrier were slow.
    pub stall_stops: u64,
    /// The same, for a holder whose claim was answered too late to renew once.
    pub late_start_stops: u64,
    pub longest_wait_ms: u64,
}

impl RenewalOutcome {
    pub fn violated(&self) -> bool {
        self.overlapping_owners + self.stale_writes + self.beyond_store_window > 0
    }
}

// Independent rates within +/-1000 ppm. Offsets never matter.
#[derive(Clone, Copy)]
struct Clock {
    base: Instant,
    ppm: i64,
}

impl Clock {
    fn at(self, real: u64) -> Instant {
        let local = u128::from(real) * (1_000_000 + self.ppm) as u128 / 1_000_000;
        self.base + Duration::from_nanos(local as u64)
    }

    // The first real time at which this clock reads at least `instant`.
    fn real(self, instant: Instant) -> u64 {
        let local = instant.saturating_duration_since(self.base).as_nanos();
        (local * 1_000_000).div_ceil((1_000_000 + self.ppm) as u128) as u64
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = mix(self.0);
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }

    fn between(&mut self, low: u64, high: u64) -> u64 {
        low + self.below(high.saturating_sub(low) + 1)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn rate(&mut self) -> i64 {
        self.below(2001) as i64 - 1000
    }
}

#[derive(Clone, Copy, Debug)]
enum Record {
    Claim,
    Renew(u64),
    Drain,
    Release,
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Directory,
    Takeover(Fence),
    Claim,
    Renew { fence: Fence, counter: u64 },
    Release(Fence),
}

#[derive(Clone, Copy, Debug)]
enum Answer {
    Directory(Option<Fence>),
    Ok,
    Leased(LeaseGrant),
    Refused,
}

#[derive(Clone, Default)]
struct TileState {
    epoch: u64,
    active: bool,
    lease: Option<StoreLease>,
}

impl TileState {
    fn fence(&self) -> Fence {
        Fence {
            tile: TileId(0),
            process: ProcessId(self.epoch),
            epoch: Epoch(self.epoch),
        }
    }

    fn holds(&self, fence: Fence, now: Instant) -> bool {
        self.epoch != 0
            && self.active
            && self.fence() == fence
            && self.lease.as_ref().is_some_and(|lease| lease.active(now))
    }
}

type Call = (usize, u64);

struct Store {
    clock: Clock,
    state: TileState,
    // Read only by the stale-view fault.
    view: TileState,
    journal: Vec<Record>,
    durable: usize,
    queue: VecDeque<(Call, Op)>,
    in_flight: Option<Vec<(Call, Answer)>>,
    // Written only by the lost-extension fault.
    deferred_drain: Option<StoreLease>,
    incarnation: u64,
    down: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Idle,
    Directory,
    Takeover,
    Claim,
    Hold,
    // Lost its lease but keeps acting. Only the ignored-deadline control.
    Zombie,
    Dead,
}

struct Process {
    clock: Clock,
    phase: Phase,
    keeper: Option<LeaseKeeper>,
    schedule: Option<RenewalSchedule>,
    ticket: Option<LeaseRequest>,
    fence: Option<Fence>,
    call: Option<u64>,
    calls: u64,
    sent: u64,
    wake: u64,
    // Something other than the store's own queue and barrier delayed this
    // process since its last grant: a partition, a crash or a slow link.
    excused: bool,
    renewed: bool,
    forceful: bool,
    graceful: bool,
    partition: (u64, u64),
}

enum Event {
    Wake(usize, u64),
    Tick(usize),
    Arrive(Call, Op),
    Reply(Call, Answer),
    BatchDone(u64),
    Crash { down_for: u64, power_loss: bool },
    Restart,
    Die(usize),
}

struct Simulation {
    setup: RenewalSetup,
    rng: Rng,
    origin: Instant,
    events: BTreeMap<(u64, u64), Event>,
    pushed: u64,
    store: Store,
    processes: Vec<Process>,
    newest_writer: u64,
    outcome: RenewalOutcome,
}

/// One tile, three processes that claim, renew, take over, pause and die, and
/// a store whose barriers stall and which crashes and restarts. Tiles run the
/// production `LeaseKeeper` and `RenewalSchedule`, the store the production
/// `StoreLease`. The checker reads only actions and the store's own state.
pub fn renewal_crash(seed: u64, setup: RenewalSetup) -> RenewalOutcome {
    let mut rng = Rng(mix(seed ^ 0xe29));
    // Room below every clock reading for the planted early recoveries.
    let origin = Instant::now() + Duration::from_secs(3600);
    let lease = setup.lease_ms * MS;
    let horizon = 12 * lease;
    let mut simulation = Simulation {
        setup,
        store: Store {
            clock: Clock {
                base: origin,
                ppm: rng.rate(),
            },
            state: TileState::default(),
            view: TileState::default(),
            journal: Vec::new(),
            durable: 0,
            queue: VecDeque::new(),
            in_flight: None,
            deferred_drain: None,
            incarnation: 0,
            down: false,
        },
        rng,
        origin,
        events: BTreeMap::new(),
        pushed: 0,
        processes: Vec::new(),
        newest_writer: 0,
        outcome: RenewalOutcome::default(),
    };
    let mut starts = [0; PROCESSES];
    for (index, start) in starts.iter_mut().enumerate() {
        let rng = &mut simulation.rng;
        *start = if index == 0 {
            rng.between(0, 20 * MS)
        } else {
            rng.between(lease / 2, horizon * 3 / 4)
        };
        let death = rng.chance(50).then(|| rng.between(*start + lease, horizon));
        let partition = if rng.chance(40) {
            let from = rng.between(*start, horizon);
            (from, from + rng.between(lease / 5, 2 * lease))
        } else {
            (0, 0)
        };
        let process = Process {
            clock: Clock {
                base: origin,
                ppm: rng.rate(),
            },
            phase: Phase::Idle,
            keeper: None,
            schedule: None,
            ticket: None,
            fence: None,
            call: None,
            calls: 0,
            sent: 0,
            wake: 0,
            excused: false,
            renewed: false,
            forceful: rng.chance(75),
            graceful: rng.chance(30),
            partition,
        };
        let first_tick = *start + rng.between(0, lease / 200);
        simulation.processes.push(process);
        simulation.push(*start, Event::Wake(index, 0));
        simulation.push(first_tick, Event::Tick(index));
        if let Some(at) = death {
            simulation.push(at, Event::Die(index));
        }
    }
    for _ in 0..simulation.rng.below(3) {
        let rng = &mut simulation.rng;
        // Half the crashes land near a start, where claims and takeovers happen.
        let at = if rng.chance(50) {
            rng.between(0, horizon)
        } else {
            starts[rng.below(PROCESSES as u64) as usize] + rng.between(0, lease)
        };
        let down_for = if rng.chance(50) {
            rng.between(MS, lease / 4)
        } else {
            rng.between(lease / 4, lease * 3 / 2)
        };
        let power_loss = rng.chance(50);
        simulation.push(
            at,
            Event::Crash {
                down_for,
                power_loss,
            },
        );
    }
    while let Some(((at, _), event)) = simulation.events.pop_first() {
        if at > horizon {
            break;
        }
        simulation.handle(at, event);
    }
    simulation.outcome
}

impl Simulation {
    fn push(&mut self, at: u64, event: Event) {
        self.pushed += 1;
        self.events.insert((at, self.pushed), event);
    }

    fn lease(&self) -> u64 {
        self.setup.lease_ms * MS
    }

    fn handle(&mut self, now: u64, event: Event) {
        match event {
            Event::Wake(index, token) => self.wake(index, token, now),
            Event::Tick(index) => self.tick(index, now),
            Event::Arrive(call, op) => self.arrive(call, op, now),
            Event::Reply(call, answer) => self.reply(call, answer, now),
            Event::BatchDone(incarnation) => self.batch_done(incarnation, now),
            Event::Crash {
                down_for,
                power_loss,
            } => self.crash(down_for, power_loss, now),
            Event::Restart => self.restart(now),
            Event::Die(index) => self.die(index, now),
        }
    }

    // --- network ---

    fn cut(&self, index: usize, now: u64) -> bool {
        let (from, until) = self.processes[index].partition;
        from <= now && now < until
    }

    fn delay(&mut self, index: usize) -> u64 {
        if self.rng.chance(90) {
            self.rng.between(MS / 20, 3 * MS)
        } else {
            self.processes[index].excused = true;
            self.rng.between(3 * MS, self.lease() / 4)
        }
    }

    fn send(&mut self, index: usize, op: Op, now: u64) {
        let process = &mut self.processes[index];
        process.calls += 1;
        process.call = Some(process.calls);
        process.sent = now;
        let call = (index, process.calls);
        if self.cut(index, now) {
            self.processes[index].excused = true;
            return;
        }
        let at = now + self.delay(index);
        self.push(at, Event::Arrive(call, op));
    }

    fn answer(&mut self, call: Call, answer: Answer, now: u64) {
        let at = now + self.delay(call.0);
        self.push(at, Event::Reply(call, answer));
    }

    fn sleep(&mut self, index: usize, until: u64) {
        self.processes[index].wake += 1;
        let token = self.processes[index].wake;
        self.push(until, Event::Wake(index, token));
    }

    // --- store ---

    fn arrive(&mut self, call: Call, op: Op, now: u64) {
        if self.store.down {
            self.processes[call.0].excused = true;
            return;
        }
        match op {
            Op::Renew { fence, counter } if self.setup.path == RenewalPath::Unjournaled => {
                if let Some(grant) = self.renew_early(fence, counter, now) {
                    self.outcome.unjournaled_grants += 1;
                    self.answer(call, Answer::Leased(grant), now);
                    return;
                }
            }
            Op::Claim if self.setup.fault == RenewalFault::UnjournaledClaim => {
                let answer = self.apply(op, now);
                self.answer(call, answer, now);
                return;
            }
            _ => {}
        }
        // Everything else, refusals included, keeps the journaled order.
        self.store.queue.push_back((call, op));
        if self.store.in_flight.is_none() {
            self.start_batch(now);
        }
    }

    fn renew_early(&mut self, fence: Fence, counter: u64, now: u64) -> Option<LeaseGrant> {
        let at = self.store.clock.at(now);
        if self.setup.fault == RenewalFault::StaleView {
            let view = &mut self.store.view;
            if !view.holds(fence, at) {
                return None;
            }
            let lease = view.lease.as_mut()?;
            if !lease.renew_unjournaled(counter, at) {
                return None;
            }
            let grant = lease.grant(fence);
            let live = &mut self.store.state;
            if live.fence() == fence
                && let Some(lease) = live.lease.as_mut()
            {
                // The planted bug answers whether or not this took effect.
                let _ = lease.renew(counter, at);
            }
            return Some(grant);
        }
        let state = &mut self.store.state;
        if !state.holds(fence, at) {
            return None;
        }
        let lease = state.lease.as_mut()?;
        lease
            .renew_unjournaled(counter, at)
            .then(|| lease.grant(fence))
    }

    fn apply(&mut self, op: Op, now: u64) -> Answer {
        let at = self.store.clock.at(now);
        let store = &mut self.store;
        match op {
            Op::Directory => {
                let fence = store.state.fence();
                Answer::Directory(store.state.holds(fence, at).then_some(fence))
            }
            Op::Claim => {
                if store
                    .state
                    .lease
                    .as_ref()
                    .is_some_and(|lease| lease.active(at))
                {
                    return Answer::Refused;
                }
                let lease = StoreLease::new(self.setup.lease_ms, at).expect("valid lease length");
                store.state.epoch += 1;
                store.state.active = true;
                let grant = lease.grant(store.state.fence());
                store.state.lease = Some(lease);
                store.journal.push(Record::Claim);
                self.outcome.claims += 1;
                Answer::Leased(grant)
            }
            Op::Renew { fence, counter } => {
                if !store.state.holds(fence, at) {
                    return Answer::Refused;
                }
                let Some(lease) = store.state.lease.as_mut() else {
                    return Answer::Refused;
                };
                if !lease.renew(counter, at) {
                    return Answer::Refused;
                }
                store.journal.push(Record::Renew(counter));
                Answer::Leased(lease.grant(fence))
            }
            Op::Takeover(fence) => {
                if !store.state.holds(fence, at) {
                    return Answer::Refused;
                }
                let Some(lease) = store.state.lease.as_mut() else {
                    return Answer::Refused;
                };
                if self.setup.fault == RenewalFault::LostExtension {
                    let mut prepared = lease.clone();
                    prepared.drain();
                    store.deferred_drain = Some(prepared);
                } else {
                    lease.drain();
                }
                store.journal.push(Record::Drain);
                Answer::Ok
            }
            Op::Release(fence) => {
                if !store.state.holds(fence, at) {
                    return Answer::Refused;
                }
                store.state.active = false;
                store.journal.push(Record::Release);
                Answer::Ok
            }
        }
    }

    fn start_batch(&mut self, now: u64) {
        let written = self.store.journal.len();
        let mut replies = Vec::new();
        while let Some((call, op)) = self.store.queue.pop_front() {
            let answer = self.apply(op, now);
            replies.push((call, answer));
        }
        // A batch without a record has no barrier, as in the store.
        let barrier = if self.store.journal.len() == written {
            0
        } else {
            let (tail_percent, low, high) = match self.setup.stall {
                Stall::Calm => (0, 0, 0),
                Stall::Tail => (15, 150, 700),
                Stall::Severe => (50, 300, 2500),
            };
            if self.rng.chance(tail_percent) {
                self.rng.between(low * MS, high * MS)
            } else {
                self.rng.between(MS, 40 * MS)
            }
        };
        self.store.in_flight = Some(replies);
        let incarnation = self.store.incarnation;
        self.push(now + barrier, Event::BatchDone(incarnation));
    }

    fn batch_done(&mut self, incarnation: u64, now: u64) {
        if self.store.down || incarnation != self.store.incarnation {
            return;
        }
        let store = &mut self.store;
        store.durable = store.journal.len();
        if let Some(prepared) = store.deferred_drain.take() {
            store.state.lease = Some(prepared);
        }
        if let Some(lease) = store.state.lease.as_mut() {
            lease.settle();
        }
        store.view = store.state.clone();
        for (call, answer) in store.in_flight.take().unwrap_or_default() {
            self.answer(call, answer, now);
        }
        if !self.store.queue.is_empty() {
            self.start_batch(now);
        }
    }

    fn crash(&mut self, down_for: u64, power_loss: bool, now: u64) {
        if self.store.down {
            return;
        }
        // A killed process keeps every written record. A power cut keeps the
        // durable prefix and any part of the unsynced tail.
        let unsynced = self.store.journal.len() - self.store.durable;
        let kept = if power_loss {
            self.rng.below(unsynced as u64 + 1) as usize
        } else {
            unsynced
        };
        let store = &mut self.store;
        store.journal.truncate(store.durable + kept);
        store.durable = store.journal.len();
        let waiting: Vec<usize> = store
            .queue
            .drain(..)
            .map(|(call, _)| call.0)
            .chain(
                store
                    .in_flight
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(call, _)| call.0),
            )
            .collect();
        store.deferred_drain = None;
        store.down = true;
        store.incarnation += 1;
        for index in waiting {
            self.processes[index].excused = true;
        }
        self.outcome.store_crashes += 1;
        self.push(now + down_for, Event::Restart);
    }

    fn restart(&mut self, now: u64) {
        self.store.clock = Clock {
            base: self.origin,
            ppm: self.rng.rate(),
        };
        let lease_ms = self.setup.lease_ms;
        let at = self.store.clock.at(now);
        let recovered_at = match self.setup.fault {
            RenewalFault::NoRestartWait => at - Duration::from_millis(lease_ms),
            RenewalFault::ShortRestartWait => at - Duration::from_millis(lease_ms / 2),
            _ => at,
        };
        let mut state = TileState::default();
        let mut durable = None;
        for record in &self.store.journal {
            match record {
                Record::Claim => {
                    state.epoch += 1;
                    state.active = true;
                    durable = Some((0, false));
                }
                Record::Renew(counter) => {
                    if let Some(lease) = durable.as_mut() {
                        lease.0 = *counter;
                    }
                }
                Record::Drain => {
                    if let Some(lease) = durable.as_mut() {
                        lease.1 = true;
                    }
                }
                Record::Release => state.active = false,
            }
        }
        if let Some((counter, draining)) = durable {
            state.lease = StoreLease::recover(lease_ms, counter, draining, recovered_at);
        }
        let before = &self.store.state;
        if before.epoch == state.epoch
            && let (Some(old), Some(new)) = (&before.lease, &state.lease)
            && old.counter() > new.counter()
        {
            self.outcome.renewals_lost += 1;
        }
        self.store.view = state.clone();
        self.store.state = state;
        self.store.down = false;
    }

    // --- tile processes ---

    fn usable(&self) -> u64 {
        self.lease() / 10 * 9
    }

    fn backoff(&mut self) -> u64 {
        let lease = self.lease();
        self.rng.between(lease / 100, lease / 25)
    }

    fn wake(&mut self, index: usize, token: u64, now: u64) {
        if self.processes[index].wake != token {
            return;
        }
        match self.processes[index].phase {
            Phase::Dead | Phase::Zombie => {}
            Phase::Idle => self.look(index, now),
            Phase::Directory | Phase::Takeover => {
                // The call timed out. Start over.
                self.processes[index].call = None;
                self.look(index, now);
            }
            Phase::Claim => {
                if self.processes[index].call.take().is_some() {
                    // An unanswered claim may still be granted. Look again.
                    self.look(index, now);
                } else {
                    self.claim(index, now);
                }
            }
            Phase::Hold => self.renew(index, now),
        }
    }

    fn look(&mut self, index: usize, now: u64) {
        let process = &mut self.processes[index];
        process.phase = Phase::Directory;
        process.keeper = None;
        process.schedule = None;
        process.fence = None;
        self.send(index, Op::Directory, now);
        self.sleep(index, now + self.usable());
    }

    fn claim(&mut self, index: usize, now: u64) {
        let margin = Duration::from_millis(self.setup.lease_ms / 10);
        let process = &mut self.processes[index];
        process.phase = Phase::Claim;
        // A failed claim never reuses its keeper.
        let mut keeper =
            LeaseKeeper::new(TileId(0), self.setup.lease_ms, margin).expect("valid margin");
        process.ticket = keeper.request(process.clock.at(now));
        process.keeper = Some(keeper);
        self.send(index, Op::Claim, now);
        self.sleep(index, now + self.usable());
    }

    // The tile's renewal loop: one serial stream on the production schedule.
    fn renew(&mut self, index: usize, now: u64) {
        let process = &mut self.processes[index];
        // A wake during a call is its call deadline. The read is abandoned.
        process.call = None;
        let at = process.clock.at(now);
        let clock = process.clock;
        let (Some(keeper), Some(schedule), Some(fence)) = (
            process.keeper.as_mut(),
            process.schedule.as_mut(),
            process.fence,
        ) else {
            return;
        };
        let Some(deadline) = keeper.deadline() else {
            return;
        };
        match schedule.advance(at, deadline) {
            RenewalAction::Expired => self.lose(index, now, true),
            RenewalAction::WaitUntil(when) => self.sleep(index, clock.real(when).max(now + 1)),
            RenewalAction::SendBefore(call_deadline) => {
                let Some(ticket) = keeper.request(at) else {
                    self.lose(index, now, true);
                    return;
                };
                process.ticket = Some(ticket);
                let op = Op::Renew {
                    fence,
                    counter: ticket.counter(),
                };
                self.send(index, op, now);
                self.sleep(index, clock.real(call_deadline).max(now + 1));
            }
        }
    }

    fn reply(&mut self, call: Call, answer: Answer, now: u64) {
        let (index, id) = call;
        let phase = self.processes[index].phase;
        if matches!(phase, Phase::Dead | Phase::Zombie) || self.processes[index].call != Some(id) {
            return;
        }
        if self.cut(index, now) {
            self.processes[index].excused = true;
            return;
        }
        let period = Duration::from_millis(self.setup.lease_ms / 4);
        let slack = Duration::from_millis(self.setup.lease_ms / 20);
        let process = &mut self.processes[index];
        process.call = None;
        let at = process.clock.at(now);
        match (phase, answer) {
            (Phase::Directory, Answer::Directory(Some(fence))) if process.forceful => {
                process.phase = Phase::Takeover;
                self.send(index, Op::Takeover(fence), now);
                self.sleep(index, now + self.usable());
            }
            (Phase::Directory | Phase::Takeover, _) => self.claim(index, now),
            (Phase::Claim, Answer::Leased(grant)) => {
                let ticket = process.ticket.take();
                let schedule = match (process.keeper.as_mut(), ticket) {
                    (Some(keeper), Some(ticket)) => keeper
                        .observe(ticket, grant, at)
                        .then(|| RenewalSchedule::new(ticket.sent(), period, slack))
                        .flatten(),
                    _ => None,
                };
                if let Some(schedule) = schedule {
                    process.phase = Phase::Hold;
                    process.schedule = Some(schedule);
                    process.fence = Some(grant.fence);
                    process.excused = false;
                    process.renewed = false;
                    self.outcome.holds += 1;
                    self.renew(index, now);
                } else {
                    // Granted too late to use. The store now holds a lease
                    // nobody acts on, and the next look takes it over.
                    process.phase = Phase::Idle;
                    self.outcome.late_claims += 1;
                    let until = now + self.backoff();
                    self.sleep(index, until);
                }
            }
            (Phase::Claim, _) => {
                let until = now + self.backoff();
                self.sleep(index, until);
            }
            (Phase::Hold, Answer::Leased(grant)) => {
                let sent = process.sent;
                let ticket = process.ticket.take();
                let observed = match (process.keeper.as_mut(), ticket) {
                    (Some(keeper), Some(ticket)) => keeper.observe(ticket, grant, at),
                    _ => false,
                };
                if observed {
                    process.excused = false;
                    process.renewed = true;
                    self.outcome.renewals += 1;
                    self.outcome.longest_wait_ms =
                        self.outcome.longest_wait_ms.max((now - sent) / MS);
                }
                self.renew(index, now);
            }
            (Phase::Hold, _) => self.lose(index, now, false),
            _ => {}
        }
    }

    fn lose(&mut self, index: usize, now: u64, expired: bool) {
        if expired {
            self.outcome.expiries += 1;
            let process = &self.processes[index];
            let store = &self.store;
            let undisturbed = !process.excused
                && !store.down
                && process.fence.is_some_and(|fence| {
                    store.state.holds(fence, store.clock.at(now))
                        && store
                            .state
                            .lease
                            .as_ref()
                            .is_some_and(|lease| !lease.draining())
                });
            if undisturbed && process.renewed {
                self.outcome.stall_stops += 1;
            } else if undisturbed {
                self.outcome.late_start_stops += 1;
            }
        }
        let restarts = self.rng.chance(60);
        let until = now + self.rng.between(self.lease() / 5, self.lease());
        let process = &mut self.processes[index];
        if let Some(keeper) = process.keeper.as_mut() {
            keeper.stop();
        }
        process.call = None;
        process.ticket = None;
        process.wake += 1;
        if self.setup.fault == RenewalFault::IgnoreDeadline {
            process.phase = Phase::Zombie;
        } else if restarts {
            process.phase = Phase::Idle;
            self.sleep(index, until);
        } else {
            process.phase = Phase::Dead;
        }
    }

    fn die(&mut self, index: usize, now: u64) {
        let process = &mut self.processes[index];
        let held = process.phase == Phase::Hold;
        process.phase = Phase::Dead;
        process.wake += 1;
        process.call = None;
        if let Some(keeper) = process.keeper.as_mut() {
            keeper.stop();
        }
        // A clean shutdown stops acting first, then tells the store.
        if let (true, true, Some(fence)) = (held, process.graceful, process.fence) {
            self.send(index, Op::Release(fence), now);
        }
    }

    // --- actions and the checker ---

    fn tick(&mut self, index: usize, now: u64) {
        let process = &self.processes[index];
        match process.phase {
            Phase::Dead => return,
            Phase::Zombie => self.act(index, now),
            Phase::Hold => {
                let open = process
                    .keeper
                    .as_ref()
                    .is_some_and(|keeper| keeper.may_act(process.clock.at(now)));
                if open || self.setup.fault == RenewalFault::IgnoreDeadline {
                    self.act(index, now);
                }
                if !open {
                    self.lose(index, now, true);
                }
            }
            _ => {}
        }
        let next = now + self.lease() / 200 + self.rng.between(0, self.lease() / 2000);
        self.push(next, Event::Tick(index));
    }

    fn would_act(&self, index: usize, now: u64) -> bool {
        let process = &self.processes[index];
        match process.phase {
            Phase::Zombie => true,
            Phase::Hold => process
                .keeper
                .as_ref()
                .and_then(LeaseKeeper::deadline)
                .is_some_and(|deadline| process.clock.at(now) < deadline),
            _ => false,
        }
    }

    fn act(&mut self, index: usize, now: u64) {
        let Some(fence) = self.processes[index].fence else {
            return;
        };
        self.outcome.actions += 1;
        let others = (0..self.processes.len())
            .filter(|other| *other != index && self.would_act(*other, now))
            .count();
        self.outcome.overlapping_owners += others as u64;
        if fence.epoch.0 < self.newest_writer {
            self.outcome.stale_writes += 1;
        } else {
            self.newest_writer = fence.epoch.0;
        }
        if !self.store.down && !self.store.state.holds(fence, self.store.clock.at(now)) {
            self.outcome.beyond_store_window += 1;
        }
    }
}
