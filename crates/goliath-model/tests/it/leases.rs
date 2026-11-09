//! Owns deterministic boundary, replay and clock-rate checks for lease authority.

use goliath_model::{Epoch, Fence, LeaseKeeper, ProcessId, StoreLease, TileId};
use std::time::{Duration, Instant, SystemTime};

#[test]
fn renewal_cadence_replays_the_failed_call_without_post_call_sleep() {
    use goliath_model::{RenewalAction, RenewalSchedule};
    let start = Instant::now();
    let at = |us| start + Duration::from_micros(us);
    let mut keeper = LeaseKeeper::new(TileId(0), 1000, Duration::from_millis(100)).unwrap();
    let claim = keeper.request(at(0)).unwrap();
    let mut store = StoreLease::new(1000, at(0)).unwrap();
    assert!(keeper.observe(claim, store.grant(fence()), at(25_756)));
    let mut cadence =
        RenewalSchedule::new(at(0), Duration::from_millis(250), Duration::from_millis(50)).unwrap();
    assert_eq!(
        cadence.advance(at(280_652), keeper.deadline().unwrap()),
        RenewalAction::SendBefore(at(850_000))
    );
    let lost = keeper.request(at(280_652)).unwrap();
    assert!(store.renew(lost.counter(), at(280_660)));
    // Socket timeout is observed only after the joint process STOP ends.
    assert!(keeper.may_act(at(716_315)));
    assert_eq!(
        cadence.advance(at(716_315), keeper.deadline().unwrap()),
        RenewalAction::SendBefore(at(850_000))
    );
    let next = keeper.request(at(716_315)).unwrap();
    assert!(store.renew(next.counter(), at(720_315)));
    assert!(keeper.observe(next, store.grant(fence()), at(721_315)));
    assert_eq!(keeper.deadline(), Some(at(1_616_315)));
    assert!(keeper.may_act(at(950_000)));
    assert_eq!(
        cadence.advance(at(721_315), keeper.deadline().unwrap()),
        RenewalAction::WaitUntil(at(966_315))
    );
}

#[test]
fn a_stop_beyond_the_deadline_still_latches_and_cannot_revive() {
    use goliath_model::{RenewalAction, RenewalSchedule};
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut keeper = LeaseKeeper::new(TileId(0), 1000, Duration::from_millis(100)).unwrap();
    let claim = keeper.request(at(0)).unwrap();
    let mut store = StoreLease::new(1000, at(0)).unwrap();
    assert!(keeper.observe(claim, store.grant(fence()), at(1)));
    let mut cadence =
        RenewalSchedule::new(at(0), Duration::from_millis(250), Duration::from_millis(50)).unwrap();
    assert!(matches!(
        cadence.advance(at(250), keeper.deadline().unwrap()),
        RenewalAction::SendBefore(_)
    ));
    let renewal = keeper.request(at(250)).unwrap();
    assert!(store.renew(renewal.counter(), at(251)));
    assert_eq!(
        cadence.advance(at(1200), keeper.deadline().unwrap()),
        RenewalAction::Expired
    );
    assert!(!keeper.observe(renewal, store.grant(fence()), at(1200)));
    assert!(!keeper.may_act(at(500)));
}

#[test]
fn immediate_failures_do_not_burst_and_slow_success_does_not_add_sleep() {
    use goliath_model::{RenewalAction, RenewalSchedule};
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut cadence =
        RenewalSchedule::new(at(0), Duration::from_millis(250), Duration::from_millis(50)).unwrap();
    let mut sent = Vec::new();
    for ms in 0..5000 {
        if matches!(
            cadence.advance(at(ms), at(5000)),
            RenewalAction::SendBefore(_)
        ) {
            sent.push(ms);
        }
    }
    for left in 0..4000 {
        assert!(
            sent.iter()
                .filter(|&&ms| left <= ms && ms < left + 1000)
                .count()
                <= 4
        );
    }
    assert!(sent.windows(2).all(|pair| pair[1] - pair[0] >= 250));
    let mut slow =
        RenewalSchedule::new(at(0), Duration::from_millis(250), Duration::from_millis(50)).unwrap();
    assert!(matches!(
        slow.advance(at(250), at(900)),
        RenewalAction::SendBefore(_)
    ));
    // A successful435ms call is overdue, not followed by another250ms sleep.
    assert!(matches!(
        slow.advance(at(685), at(1150)),
        RenewalAction::SendBefore(_)
    ));
    assert_eq!(
        slow.advance(at(690), at(1150)),
        RenewalAction::WaitUntil(at(935))
    );
}

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

#[test]
fn only_a_settled_lease_is_renewed_without_a_journal_record() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut store = StoreLease::new(1000, at(0)).unwrap();
    assert!(!store.settled());
    // The claim is still behind its barrier: nothing may extend it early.
    assert!(!store.renew_unjournaled(1, at(10)));
    assert_eq!(store.counter(), 0);
    assert_eq!(store.deadline(), at(1000));
    // The journaled path is unchanged for an unsettled lease.
    assert!(store.renew(1, at(20)));
    store.settle();
    assert!(store.renew_unjournaled(2, at(300)));
    assert_eq!(store.deadline(), at(1300));
    // Replay, drain and expiry are refused exactly as on the journaled path.
    assert!(!store.renew_unjournaled(2, at(310)));
    assert!(!store.renew_unjournaled(3, at(1300)));
    let mut draining = store.clone();
    draining.drain();
    assert!(!draining.renew_unjournaled(3, at(320)));
    assert_eq!(draining.deadline(), at(1300));
    // A crash loses counter 2. Recovery is settled and waits L from restart,
    // which is later than the lost grant's own expiry.
    let recovered = StoreLease::recover(1000, 1, false, at(400)).unwrap();
    assert!(recovered.settled());
    assert!(recovered.deadline() >= store.deadline());
}
