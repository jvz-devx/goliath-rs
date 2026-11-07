//! Owns freeze-cut deltas, physical append order, durable prefixes and compaction.

use crate::{Delta, Fence, Image};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct CutId(pub u64);

#[derive(Clone, Debug)]
pub struct Cut {
    pub id: CutId,
    pub fence: Fence,
    changes: Vec<Delta>,
}

#[derive(Clone, Debug)]
struct Record {
    cut: Cut,
    valid: bool,
    position: u64,
}

#[derive(Clone, Debug)]
pub struct CutLog {
    captured: Image,
    next: CutId,
    base: Image,
    base_id: CutId,
    head: Image,
    head_id: CutId,
    records: Vec<Record>,
    flushed: u64,
    written: u64,
    pub stale_accepted: u64,
}

impl CutLog {
    pub fn new(image: Image) -> Self {
        Self {
            captured: image,
            next: CutId(0),
            base: image,
            base_id: CutId(0),
            head: image,
            head_id: CutId(0),
            records: Vec::new(),
            flushed: 0,
            written: 0,
            stale_accepted: 0,
        }
    }

    /// The immutable image stands for references captured together at freeze.
    /// Each delta depends on the previous capture, not the last worker to finish.
    pub fn capture(&mut self, image: Image, fence: Fence, skip_chest: bool) -> Cut {
        self.next.0 += 1;
        let mut saved = image;
        if skip_chest {
            for (places, previous) in saved.items.iter_mut().zip(self.captured.items) {
                places[0] = previous[0];
            }
        }
        let cut = Cut {
            id: self.next,
            fence,
            changes: Delta::between(self.captured, saved),
        };
        self.captured = image;
        cut
    }

    pub fn append(&mut self, cut: Cut, current: Fence, disable_fence: bool) -> bool {
        if cut.fence != current {
            if !disable_fence {
                return false;
            }
            self.stale_accepted += 1;
        }
        self.written += 1;
        self.records.push(Record {
            cut,
            valid: true,
            position: self.written,
        });
        true
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    pub fn covers(&self, id: CutId) -> bool {
        self.head_id >= id
    }

    /// A barrier covers only records already submitted when it started.
    /// A hole in capture order blocks publication even if later records flushed.
    pub fn flush(&mut self, end: u64) {
        self.flushed = self.flushed.max(end.min(self.written));
        self.replay(false);
    }

    fn replay(&mut self, past_torn: bool) {
        self.head = self.base;
        self.head_id = self.base_id;
        let torn = self
            .records
            .iter()
            .find(|r| !r.valid)
            .map_or(u64::MAX, |r| r.position);
        while let Some(record) = self.records.iter().find(|r| {
            r.position <= self.flushed
                && (past_torn || r.position < torn)
                && r.valid
                && r.cut.id.0 == self.head_id.0 + 1
        }) {
            for delta in &record.cut.changes {
                delta.apply(&mut self.head);
            }
            self.head_id = record.cut.id;
        }
        if past_torn {
            for record in &self.records {
                if record.valid && record.cut.id > self.head_id {
                    for delta in &record.cut.changes {
                        delta.apply(&mut self.head);
                    }
                    self.head_id = record.cut.id;
                }
            }
        }
    }

    pub fn compaction(&self) -> (CutId, Image) {
        (self.head_id, self.head)
    }

    pub fn compact(&mut self, id: CutId, image: Image) {
        if id < self.base_id || id > self.head_id {
            return;
        }
        self.base_id = id;
        self.base = image;
        // Retain newer records even when they appended before older captures.
        self.records.retain(|r| r.cut.id > id);
        self.replay(false);
    }

    /// Simulate an interrupted final write with a valid successor in the OS
    /// cache. Neither the torn frame nor the successor has crossed a barrier.
    pub fn tear_unflushed_tail(&mut self) {
        if let Some(record) = self.records.iter_mut().find(|r| r.position > self.flushed) {
            record.valid = false;
        }
    }

    pub fn recover(&mut self, past_torn: bool) -> Image {
        if past_torn {
            // Deliberate bug: scan valid cached frames beyond a torn record.
            self.flushed = self.written;
            self.replay(true);
        } else {
            self.records.retain(|r| r.position <= self.flushed);
            self.replay(false);
        }
        self.records.retain(|r| r.valid && r.cut.id <= self.head_id);
        self.written = self.flushed;
        self.next = self.head_id;
        self.captured = self.head;
        self.head
    }
}
