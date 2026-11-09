//! Owns FIFO retry-receipt retention in mutation acceptance order.

use crate::{Command, RequestId, Response};
use std::collections::{BTreeMap, VecDeque};

/// Qualified measured-rate budget at 650 and 1,050 players, not an all-time retry promise.
pub(crate) const CAPACITY: usize = 9_216;

#[derive(Default)]
pub(crate) struct Receipts {
    entries: BTreeMap<RequestId, (Command, Response)>,
    order: VecDeque<RequestId>,
}

impl Receipts {
    /// Blob heap only, excluding map/enum overhead. Sample outside the hot path.
    pub(crate) fn blob_bytes(&self) -> (usize, usize) {
        let mut len = 0;
        let mut capacity = 0;
        for (command, response) in self.entries.values() {
            let blob = match &command.operation {
                crate::Operation::Create { blob, .. }
                | crate::Operation::Commit { blob, .. }
                | crate::Operation::CreateUuid { blob, .. }
                | crate::Operation::CommitUuid { blob, .. } => Some(blob),
                _ => None,
            };
            if let Some(blob) = blob {
                len += blob.bytes.len();
                capacity += blob.bytes.capacity();
            }
            if let Response::Player(Some(record)) = response {
                len += record.inventory.bytes.len();
                capacity += record.inventory.bytes.capacity();
            }
        }
        (len, capacity)
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn get(&self, id: &RequestId) -> Option<&(Command, Response)> {
        self.entries.get(id)
    }

    pub(crate) fn insert(&mut self, id: RequestId, receipt: (Command, Response)) {
        if self.entries.insert(id, receipt).is_some() {
            // Preserve the existing position if a snapshot repeats an identity.
            return;
        }
        self.order.push_back(id);
        if self.order.len() > CAPACITY {
            let oldest = self.order.pop_front().expect("nonempty receipt FIFO");
            self.entries
                .remove(&oldest)
                .expect("receipt FIFO owns entry");
        }
    }

    /// Snapshot iteration order is part of the eviction/recovery contract.
    pub(crate) fn values(&self) -> impl Iterator<Item = &(Command, Response)> {
        self.order
            .iter()
            .map(|id| self.entries.get(id).expect("receipt FIFO owns entry"))
    }
}

#[cfg(test)]
mod tests {
    //! Owns exact receipt-window boundaries independently of domain authorization.

    use super::*;
    use crate::{Operation, TileId};

    fn add(receipts: &mut Receipts, id: u128) {
        let id = RequestId(id);
        receipts.insert(
            id,
            (
                Command {
                    id,
                    operation: Operation::Claim(TileId(0)),
                },
                Response::Ok,
            ),
        );
    }

    #[test]
    fn boundary_is_acceptance_order_not_request_id_order_or_lookup_order() {
        let mut receipts = Receipts::default();
        let oldest = u128::MAX;
        add(&mut receipts, oldest);
        for id in 1..CAPACITY as u128 {
            add(&mut receipts, id);
        }
        assert_eq!(receipts.len(), CAPACITY);
        assert!(receipts.get(&RequestId(oldest)).is_some());
        assert!(receipts.get(&RequestId(1)).is_some());
        // A hit does not make the original request younger.
        assert!(receipts.get(&RequestId(oldest)).is_some());
        add(&mut receipts, CAPACITY as u128);
        assert_eq!(receipts.len(), CAPACITY);
        assert!(receipts.get(&RequestId(oldest)).is_none());
        assert_eq!(receipts.values().next().unwrap().0.id, RequestId(1));
    }
}
