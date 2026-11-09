//! Owns the blocking player-store adapter, durable journal and bounded wire protocol.
//!
//! E16 contract: claim with `Operation::ClaimLease`, not legacy `Claim`. Drive a
//! `LeaseKeeper` with original-send-time tickets, or use `Client::claim_lease` and
//! `Client::renew_lease`. The caller owns scheduling. No library thread renews it.
//! Gate cut append, flush and install, player actions, explicit imports and normal
//! returning logins on `keeper.may_act(Instant::now())`, at the side effect itself.
//! Cancel queued work and fail closed at its local deadline, even while isolated.
//! Bound critical work so it completes before that deadline. A store lookup
//! alone cannot fence an asynchronous cut writer.
//!
//! `BeginTakeover(current_fence)` durably prevents more renewals. It does not
//! shorten the old lease or authorize a replacement. Retry `ClaimLease` until
//! the store's last grant + L passes. Release does not shorten this wait either.
//! Restart reconstructs every durable lease as possibly live for a full L from
//! the end of recovery. Clock and Directory reads never grant local authority.
//! `CheckLease` validates activation but does not renew a local keeper.
//!
//! Old wire tags and unleased tiles retain their behavior. Once a tile uses
//! leases, legacy Claim and Heartbeat cannot bypass them. Upgrading a live
//! legacy holder is refused. Stop and release it before the first leased claim.
//! Claims, drains and releases share the existing durable journal.
//!
//! E29: a granted renewal of a lease whose claim is already durable is answered
//! by `Renewer::renew` without a journal record or barrier. A crash loses it,
//! and the full-L restart wait above covers the lost grant. Refusals, retries
//! the journal remembers and every other operation still wait for the barrier.
//! After a restart the directory's `heartbeat` may be behind the tile's counter
//! until its next renewal.
//!
//! Retry receipts retain the latest 9,216 accepted mutations in FIFO order.
//! A matching eligible retry in that window returns its original response.
//! After eviction the operation is evaluated again, still fenced and guarded.
//! Unknown results are never evidence that a mutation aborted. Caller-driven
//! resubmissions beyond one bounded Client call may lose an accurate receipt.

mod client;
mod codec;
mod durable;
mod engine;
mod journal;
mod receipts;
mod state;

pub const RECEIPT_CAPACITY: usize = receipts::CAPACITY;

pub use client::{CallTiming, Client, request_id};
pub use codec::{read_request, read_response, write_request, write_response};
pub use durable::SyncMode;
pub use engine::{Engine, Renewer, StoreResources};
pub use goliath_model::{Epoch, Fence, Generation, OwnershipCas, ProcessId, Record, TileId};
pub use goliath_model::{LeaseGrant, LeaseKeeper, LeaseRequest, RenewalAction, RenewalSchedule};
pub use journal::{Compaction, CompactionReady};
pub use state::{
    Blob, ClusterClock, Command, DirectoryEntry, Operation, PlayerKey, RequestId, Response,
    StoreFault,
};
