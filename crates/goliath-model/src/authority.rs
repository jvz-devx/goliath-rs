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
        !self.fencing || accepts_fence(Some(self.current(fence.tile)), fence)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record<B = [u8; ITEMS]> {
    pub generation: Generation,
    pub owner: TileId,
    /// Entry inventory, not a second authoritative copy during normal play.
    pub inventory: B,
}

/// The ownership part of a handover, independent of player-state encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnershipCas {
    pub generation: Generation,
    pub source: Fence,
    pub target: Fence,
}

pub fn accepts_fence(current: Option<Fence>, supplied: Fence) -> bool {
    current == Some(supplied)
}

impl OwnershipCas {
    pub fn check_fences(
        self,
        source: Option<Fence>,
        target: Option<Fence>,
    ) -> Result<(), StoreError> {
        if !accepts_fence(source, self.source) || !accepts_fence(target, self.target) {
            return Err(StoreError::StaleFence);
        }
        Ok(())
    }

    /// Checks both fences before even recognizing an idempotent delivery.
    pub fn commit<B: Clone + PartialEq>(
        self,
        record: &mut Record<B>,
        inventory: &B,
        source: Option<Fence>,
        target: Option<Fence>,
    ) -> Result<bool, StoreError> {
        self.check_fences(source, target)?;
        let next = self
            .generation
            .0
            .checked_add(1)
            .ok_or(StoreError::GenerationConflict)?;
        if self.source.tile == self.target.tile {
            return Err(StoreError::GenerationConflict);
        }
        if record.generation == Generation(next) && record.owner == self.target.tile {
            return if record.inventory == *inventory {
                Ok(false)
            } else {
                Err(StoreError::GenerationConflict)
            };
        }
        if record.generation != self.generation || record.owner != self.source.tile {
            return Err(StoreError::GenerationConflict);
        }
        *record = Record {
            generation: Generation(next),
            owner: self.target.tile,
            inventory: inventory.clone(),
        };
        Ok(true)
    }
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
        let cas = OwnershipCas {
            generation: handover.generation,
            source: handover.source_fence,
            target: handover.target_fence,
        };
        // The simulator's explicit mutation substitutes the supplied fences.
        let source = if directory.fencing {
            directory.current(handover.source)
        } else {
            cas.source
        };
        let target = if directory.fencing {
            directory.current(handover.target)
        } else {
            cas.target
        };
        let result = cas.commit(
            &mut self.records[handover.player.0],
            &handover.inventory,
            Some(source),
            Some(target),
        );
        if result.is_err() {
            self.rejected += 1;
        }
        result
    }
}
