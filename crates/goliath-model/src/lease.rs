//! Owns monotonic lease windows and caller-driven local authorization deadlines.

use crate::{Fence, TileId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A durable grant. Counter zero identifies the initial claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseGrant {
    pub fence: Fence,
    pub lease_ms: u64,
    pub counter: u64,
}

/// Store-local lease time. Never serialize an Instant or compare clocks across hosts.
#[derive(Clone, Debug)]
pub struct StoreLease {
    lease_ms: u64,
    counter: u64,
    draining: bool,
    deadline: Instant,
}

impl StoreLease {
    pub fn new(lease_ms: u64, now: Instant) -> Option<Self> {
        Self::recover(lease_ms, 0, false, now)
    }

    /// Recovery waits a full L again, even if the previous store was down briefly.
    pub fn recover(lease_ms: u64, counter: u64, draining: bool, now: Instant) -> Option<Self> {
        if !(1..=60_000).contains(&lease_ms) {
            return None;
        }
        Some(Self {
            lease_ms,
            counter,
            draining,
            deadline: now.checked_add(Duration::from_millis(lease_ms))?,
        })
    }

    pub fn lease_ms(&self) -> u64 {
        self.lease_ms
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }

    pub fn draining(&self) -> bool {
        self.draining
    }

    pub fn active(&self, now: Instant) -> bool {
        now < self.deadline
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Force never shortens the last grant. It only prevents further extensions.
    pub fn drain(&mut self) {
        self.draining = true;
    }

    pub fn renew(&mut self, counter: u64, now: Instant) -> bool {
        if !self.active(now) || self.draining || counter <= self.counter {
            return false;
        }
        let Some(deadline) = now.checked_add(Duration::from_millis(self.lease_ms)) else {
            return false;
        };
        self.counter = counter;
        self.deadline = deadline;
        true
    }

    pub fn grant(&self, fence: Fence) -> LeaseGrant {
        LeaseGrant {
            fence,
            lease_ms: self.lease_ms,
            counter: self.counter,
        }
    }
}

/// Retain this ticket across retries. A reply never starts a new local window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseRequest {
    sent: Instant,
    counter: u64,
}

impl LeaseRequest {
    pub fn counter(self) -> u64 {
        self.counter
    }

    pub fn sent(self) -> Instant {
        self.sent
    }
}

/// No threads, IO, wall clock or automatic renewal. One keeper per process claim.
///
/// Call `request` before sending a fresh claim or renewal command. Retain that
/// command's request ID and ticket across all transport retries. Feed its real
/// reply to `observe`. Counter zero is the claim, subsequent counters renew it.
/// A superseded, duplicated, late or mismatched reply cannot extend authority.
/// Once a granted local deadline passes, retire this keeper and claim anew.
///
/// Let s be real request-send time and g >= s the store's grant time. With
/// client/store clock-rate errors bounded by rc/rs, the latest local stop is
/// s + (L - margin)/(1 - rc). The earliest store expiry is g + L/(1 + rs).
/// The keeper stops first if margin >= L * (rc + rs)/(1 + rs). Late grants
/// increase g and therefore only shorten usable authority. With L=1000ms,
/// margin=100ms and rc=rs=0.001, stop <= s+900.901ms < g+999.001ms.
/// These bounds require advancing monotonic clocks. After host suspension or
/// a clock anomaly, stop the keeper before processing any queued side effect.
///
/// E12 must gate cut append, flush and install, player actions, import placement,
/// and ordinary returning logins on `may_act(Instant::now())`. Check at the actual
/// side effect, not just when work is queued. Cancel queued work on expiry.
/// Critical work must finish before `deadline()`, including pauses and IO.
/// A process unable to meet that bound needs storage-level fencing instead.
pub struct LeaseKeeper {
    tile: TileId,
    lease_ms: u64,
    margin: Duration,
    fence: Option<Fence>,
    deadline: Option<Instant>,
    pending: Option<LeaseRequest>,
    next_counter: Option<u64>,
    stopped: AtomicBool,
}

impl LeaseKeeper {
    pub fn new(tile: TileId, lease_ms: u64, margin: Duration) -> Option<Self> {
        if !(1..=60_000).contains(&lease_ms)
            || margin.is_zero()
            || margin >= Duration::from_millis(lease_ms)
        {
            return None;
        }
        Some(Self {
            tile,
            lease_ms,
            margin,
            fence: None,
            deadline: None,
            pending: None,
            next_counter: Some(0),
            stopped: AtomicBool::new(false),
        })
    }

    pub fn request(&mut self, now: Instant) -> Option<LeaseRequest> {
        if self.stopped.load(Ordering::Relaxed) || (self.fence.is_some() && !self.may_act(now)) {
            self.stop();
            return None;
        }
        let counter = self.next_counter?;
        // A failed initial claim needs a new keeper, never a counter-one claim.
        if self.fence.is_none() && counter != 0 {
            return None;
        }
        self.next_counter = counter.checked_add(1);
        let request = LeaseRequest { sent: now, counter };
        self.pending = Some(request);
        Some(request)
    }

    pub fn observe(&mut self, request: LeaseRequest, grant: LeaseGrant, now: Instant) -> bool {
        if self.pending != Some(request) {
            return false;
        }
        self.pending = None;
        if self.stopped.load(Ordering::Relaxed) || (self.fence.is_some() && !self.may_act(now)) {
            self.stop();
            return false;
        }
        let usable = Duration::from_millis(self.lease_ms) - self.margin;
        let Some(deadline) = request.sent.checked_add(usable) else {
            return false;
        };
        if grant.fence.tile != self.tile
            || grant.lease_ms != self.lease_ms
            || grant.counter != request.counter
            || now < request.sent
            || now >= deadline
            || self.fence.is_some_and(|fence| fence != grant.fence)
        {
            return false;
        }
        self.fence = Some(grant.fence);
        self.deadline = Some(deadline);
        true
    }

    pub fn may_act(&self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            // An observed expiry is permanent, even if a faulty caller later
            // supplies an older Instant. Read-only action paths can latch it too.
            self.stopped.store(true, Ordering::Relaxed);
        }
        !self.stopped.load(Ordering::Relaxed)
            && self.deadline.is_some_and(|deadline| now < deadline)
    }

    pub fn fence(&self) -> Option<Fence> {
        self.fence
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.pending = None;
    }
}
