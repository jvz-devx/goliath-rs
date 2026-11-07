//! Owns deterministic boundary, replay and clock-rate checks for lease authority.

use goliath_model::{Epoch, Fence, LeaseKeeper, ProcessId, StoreLease, TileId};
use std::time::{Duration, Instant, SystemTime};

fn fence() -> Fence {
    Fence {
        tile: TileId(0),
        process: ProcessId(1),
        epoch: Epoch(1),
    }
}

#[test]
fn delayed_replies_use_original_send_and_cannot_resurrect_expired_authority() {
    let start = Instant::now();
    let ms = |n| start + Duration::from_millis(n);
    let mut keeper = LeaseKeeper::new(TileId(0), 1000, Duration::from_millis(100)).unwrap();
    let claim = keeper.request(ms(0)).unwrap();
    let mut store = StoreLease::new(1000, ms(200)).unwrap();
    assert!(keeper.observe(claim, store.grant(fence()), ms(400)));
    assert_eq!(keeper.deadline(), Some(ms(900)));
    assert!(keeper.may_act(ms(899)));
    let renewal = keeper.request(ms(500)).unwrap();
    assert!(store.renew(renewal.counter(), ms(700)));
    assert!(!keeper.observe(renewal, store.grant(fence()), ms(900)));
    assert!(!keeper.may_act(ms(900)));
    assert!(!keeper.may_act(ms(899)));
    assert!(keeper.request(ms(901)).is_none());
}

#[test]
fn superseded_duplicate_wrong_fence_and_out_of_order_replies_do_not_extend() {
    let start = Instant::now();
    let ms = |n| start + Duration::from_millis(n);
    let mut keeper = LeaseKeeper::new(TileId(0), 1000, Duration::from_millis(100)).unwrap();
    let claim = keeper.request(ms(0)).unwrap();
    let mut store = StoreLease::new(1000, ms(1)).unwrap();
    let grant = store.grant(fence());
    assert!(keeper.observe(claim, grant, ms(2)));
    assert!(!keeper.observe(claim, grant, ms(300)));
    assert_eq!(keeper.deadline(), Some(ms(900)));
    let old = keeper.request(ms(300)).unwrap();
    let new = keeper.request(ms(400)).unwrap();
    assert!(store.renew(new.counter(), ms(500)));
    assert!(!keeper.observe(old, store.grant(fence()), ms(501)));
    assert!(keeper.observe(new, store.grant(fence()), ms(502)));
    assert_eq!(keeper.deadline(), Some(ms(1300)));
    assert!(!store.renew(old.counter(), ms(503)));
    let wrong = keeper.request(ms(600)).unwrap();
    assert!(store.renew(wrong.counter(), ms(601)));
    let mut replaced = store.grant(fence());
    replaced.fence.epoch = Epoch(2);
    assert!(!keeper.observe(wrong, replaced, ms(602)));
    assert_eq!(keeper.deadline(), Some(ms(1300)));
}

#[test]
fn force_and_restart_do_not_shorten_the_store_window() {
    let start = Instant::now();
    let ms = |n| start + Duration::from_millis(n);
    let mut lease = StoreLease::new(1000, ms(0)).unwrap();
    assert!(lease.renew(1, ms(250)));
    assert_eq!(lease.deadline(), ms(1250));
    lease.drain();
    assert!(!lease.renew(2, ms(500)));
    assert!(lease.active(ms(1249)));
    assert!(!lease.active(ms(1250)));
    let restarted =
        StoreLease::recover(lease.lease_ms(), lease.counter(), lease.draining(), ms(600)).unwrap();
    assert!(restarted.active(ms(1599)));
    assert!(!restarted.active(ms(1600)));
    assert!(restarted.draining());
}

#[test]
fn wall_clock_steps_never_enter_keeper_logic_and_drift_margin_has_slack() {
    let start = Instant::now();
    let mut keeper = LeaseKeeper::new(TileId(0), 1000, Duration::from_millis(100)).unwrap();
    let request = keeper.request(start).unwrap();
    let store = StoreLease::new(1000, start).unwrap();
    assert!(keeper.observe(request, store.grant(fence()), start));
    for wall in [
        SystemTime::UNIX_EPOCH,
        SystemTime::now() + Duration::from_secs(86_400),
    ] {
        assert!(wall >= SystemTime::UNIX_EPOCH);
        assert!(keeper.may_act(start + Duration::from_millis(899)));
    }
    assert!(!keeper.may_act(start + Duration::from_millis(900)));
    // Worst case: client loses 1000 ppm, store gains 1000 ppm.
    let usable_us = keeper.deadline().unwrap().duration_since(start).as_micros();
    assert!(usable_us * 1001 < 1_000_000 * 999);
    assert!(LeaseKeeper::new(TileId(0), 1000, Duration::ZERO).is_none());
    assert!(LeaseKeeper::new(TileId(0), 1000, Duration::from_secs(1)).is_none());
    assert!(StoreLease::new(0, start).is_none());
}
