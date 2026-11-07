//! Owns the partitioned-old-process schedule and its independent authority checker.

use crate::scenario::mix;
use goliath_model::{Cluster, LeaseKeeper, Place, PlayerId, Policy, StoreLease, Tile, TileId};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseMode {
    Enforce,
    IgnoreDeadline,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeaseOutcome {
    pub stale_appends: u64,
    pub stale_flushes: u64,
    pub stale_installs: u64,
    pub stale_logins: u64,
    pub overlapping_owners: u64,
    pub duplicate_identities: u64,
    pub missing_identities: u64,
    pub rejected_actions: u64,
    pub old_ticks_after_deadline: u64,
    pub takeover_ms: u64,
    pub restarted: bool,
}

impl LeaseOutcome {
    pub fn violated(&self) -> bool {
        self.stale_appends
            + self.stale_flushes
            + self.stale_installs
            + self.stale_logins
            + self.overlapping_owners
            + self.duplicate_identities
            + self.missing_identities
            > 0
    }
}

// Separate clock rates, each within +/-1000 ppm. Offsets are irrelevant.
fn clock(start: Instant, real_ms: u64, ppm: i64) -> Instant {
    let micros = (i128::from(real_ms) * i128::from(1_000_000 + ppm) / 1000) as u64;
    start + Duration::from_micros(micros)
}

/// P5's existing coherent log and recovery run beside a still-live old process.
/// Each process checks its own fence file only. No oracle supplies the current
/// store fence to an old append. The checker sees both independent processes.
pub fn lease_partition(seed: u64, lease_ms: u64, mode: LeaseMode) -> LeaseOutcome {
    let random = mix(seed);
    let start = Instant::now();
    let client_ppm = (random % 2001) as i64 - 1000;
    let store_ppm = (mix(random) % 2001) as i64 - 1000;
    let client_at = |at| clock(start, at, client_ppm);
    let store_at = |at| clock(start, at, store_ppm);
    let margin = Duration::from_millis(lease_ms / 10);
    let mut cluster = Cluster::new(Policy::P5);
    let tile = TileId(0);
    let old_fence = cluster.directory.current(tile);
    let mut old = cluster.tiles[0].clone();
    let mut lease = StoreLease::new(lease_ms, store_at(0)).expect("valid L");
    let mut keeper = LeaseKeeper::new(tile, lease_ms, margin).expect("valid margin");
    let claim = keeper.request(client_at(0)).expect("claim ticket");
    assert!(keeper.observe(claim, lease.grant(old_fence), client_at(1)));

    // Take an item and make the coherent donor/inventory state durable. Both
    // generations recover this same cut, so ignoring expiry exposes two copies.
    old.live.items[0][Place::Chest.index()] = 0;
    old.live.items[0][Place::Inventory(PlayerId(0)).index()] = 1;
    let cut = old.cut_log.capture(old.live, old_fence, false);
    assert!(old.cut_log.append(cut, old_fence, false));
    old.cut_log.flush(old.cut_log.written());
    let durable = old.cut_log.clone();

    let sent = lease_ms / 5 + random % (lease_ms / 10);
    let received = sent + mix(random + 1) % (lease_ms / 10 + 1);
    let reply = received + mix(random + 2) % (lease_ms / 10 + 1);
    let renewal = keeper.request(client_at(sent)).expect("renew ticket");
    assert!(lease.renew(renewal.counter(), store_at(received)));
    assert!(keeper.observe(renewal, lease.grant(old_fence), client_at(reply)));
    let partition = reply + 1;
    // A captured cut is queued before the partition. Append is a separate event.
    let queued = old.cut_log.capture(old.live, old_fence, false);
    lease.drain();
    assert!(!lease.renew(renewal.counter() + 1, store_at(partition)));
    let mut outcome = LeaseOutcome {
        restarted: random & 1 != 0,
        ..LeaseOutcome::default()
    };
    if outcome.restarted {
        let restart = partition + lease_ms / 4;
        lease = StoreLease::recover(lease_ms, lease.counter(), true, store_at(restart))
            .expect("restart lease");
    }

    // Replacement probes until the store's own lease says it may claim. This
    // includes the exact boundary, rather than using the client's deadline.
    let takeover = (partition..=partition + lease_ms * 3)
        .find(|at| !lease.active(store_at(*at)))
        .expect("lease expires");
    outcome.takeover_ms = takeover;
    let new_fence = cluster.directory.replace(tile);
    let mut replacement_log = durable;
    let recovered = replacement_log.recover(false);
    let mut replacement = Tile::new(recovered);
    let mut new_keeper = LeaseKeeper::new(tile, lease_ms, margin).expect("replacement keeper");
    let request = new_keeper
        .request(client_at(takeover))
        .expect("replacement claim");
    let new_lease = StoreLease::new(lease_ms, store_at(takeover)).expect("new store lease");
    assert!(new_keeper.observe(request, new_lease.grant(new_fence), client_at(takeover + 1)));

    // The old process keeps simulating across expiry. Captured work, fresh cuts,
    // flushes, installs and returning logins all try to cross the authority gate.
    let mut queued = Some(queued);
    for at in [
        partition,
        takeover - 1,
        takeover,
        takeover + 1,
        takeover + 25,
        takeover + 100,
    ] {
        let current = if at < takeover { old_fence } else { new_fence };
        let may_act = mode == LeaseMode::IgnoreDeadline || keeper.may_act(client_at(at));
        if !keeper.may_act(client_at(at)) {
            outcome.old_ticks_after_deadline += 1;
        }
        if !may_act {
            outcome.rejected_actions += 4;
            continue;
        }
        let cut = if at >= takeover {
            queued
                .take()
                .unwrap_or_else(|| old.cut_log.capture(old.live, old_fence, false))
        } else {
            // Keep capture order intact while the pre-partition cut is paused.
            old.cut_log.capture(old.live, old_fence, false)
        };
        assert!(old.cut_log.append(cut, old_fence, false));
        old.cut_log.flush(old.cut_log.written());
        old.live = old.cut_log.recover(false);
        let stale = u64::from(current != old_fence || !lease.active(store_at(at)));
        outcome.stale_appends += stale;
        outcome.stale_flushes += stale;
        outcome.stale_installs += stale;
        // Ordinary returning login with a matching saved generation. It needs no
        // Create or CAS call, exactly the path that failed in E13-2.
        outcome.stale_logins += stale;
        if at >= takeover && new_keeper.may_act(client_at(at)) {
            outcome.overlapping_owners += 1;
            for (left, right) in old.live.items.iter().zip(replacement.live.items) {
                let total = left.iter().map(|n| u64::from(*n)).sum::<u64>()
                    + right.iter().map(|n| u64::from(*n)).sum::<u64>();
                if total > 1 {
                    outcome.duplicate_identities += 1;
                }
            }
        }
    }
    // Replacement recovery retains all original identities exactly once.
    let cut = replacement
        .cut_log
        .capture(replacement.live, new_fence, false);
    assert!(replacement.cut_log.append(cut, new_fence, false));
    replacement.cut_log.flush(replacement.cut_log.written());
    cluster.tiles[0] = replacement;
    cluster.reconcile(tile);
    let audit = cluster.audit();
    outcome.missing_identities += audit.missing_identities;
    outcome.duplicate_identities += audit.duplicate_identities;
    outcome
}
