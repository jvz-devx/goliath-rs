//! Owns tile epochs and linearizable player generation compare-and-swap.

use crate::tile::{ITEMS, PLAYERS, TILES};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TileId(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PlayerId(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Epoch(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Generation(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fence {
    pub tile: TileId,
    pub process: ProcessId,
    pub epoch: Epoch,
}

#[derive(Clone, Debug)]
pub struct Directory {
    current: [Fence; TILES],
    pub fencing: bool,
}

impl Default for Directory {
    fn default() -> Self {
        Self {
            current: std::array::from_fn(|tile| Fence {
                tile: TileId(tile),
                process: ProcessId(tile as u64),
                epoch: Epoch(0),
            }),
            fencing: true,
        }
    }
}

impl Directory {
    pub fn current(&self, tile: TileId) -> Fence {
        self.current[tile.0]
    }

    pub fn replace(&mut self, tile: TileId) -> Fence {
        let old = self.current[tile.0];
        let new = Fence {
            tile,
            process: ProcessId(old.process.0 + TILES as u64),
            epoch: Epoch(old.epoch.0 + 1),
        };
        self.current[tile.0] = new;
        new
    }

    pub fn accepts(&self, fence: Fence) -> bool {
        !self.fencing || self.current(fence.tile) == fence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub generation: Generation,
    pub owner: TileId,
    /// Entry inventory, not a second authoritative copy during normal play.
    pub inventory: [u8; ITEMS],
}

#[derive(Clone, Debug)]
pub struct Store {
    pub records: [Record; PLAYERS],
    pub rejected: u64,
    pub stale_accepted: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    StaleFence,
    GenerationConflict,
}

impl Store {
    pub fn commit(
        &mut self,
        directory: &Directory,
        handover: &crate::Handover,
    ) -> Result<bool, StoreError> {
        if !directory.accepts(handover.source_fence) || !directory.accepts(handover.target_fence) {
            self.rejected += 1;
            return Err(StoreError::StaleFence);
        }
        // Record the planted fencing bug independently of the acceptance gate.
        if directory.current(handover.source) != handover.source_fence
            || directory.current(handover.target) != handover.target_fence
        {
            self.stale_accepted += 1;
        }
        let record = &mut self.records[handover.player.0];
        if record.generation == Generation(handover.generation.0 + 1)
            && record.owner == handover.target
        {
            return Ok(false); // Idempotent duplicate delivery, no second mutation.
        }
        if record.generation != handover.generation || record.owner != handover.source {
            self.rejected += 1;
            return Err(StoreError::GenerationConflict);
        }
        *record = Record {
            generation: Generation(record.generation.0 + 1),
            owner: handover.target,
            inventory: handover.inventory,
        };
        Ok(true)
    }
}
