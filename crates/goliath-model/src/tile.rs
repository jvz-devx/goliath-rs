//! Owns coherent tile images, atomic checkpoint installation and durable deltas.

use crate::{Epoch, Fence, Generation, PlayerId};

pub const TILES: usize = 3;
pub const PLAYERS: usize = 3;
pub const ITEMS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    Chest,
    Ground,
    Inventory(PlayerId),
}

impl Place {
    pub fn index(self) -> usize {
        match self {
            Self::Chest => 0,
            Self::Ground => 1,
            Self::Inventory(p) => p.0 + 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Actor {
    pub generation: Generation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct Image {
    /// Counts, not a single location: the checker must see copies in one tile too.
    pub items: [[u8; PLAYERS + 2]; ITEMS],
    pub players: [Option<Actor>; PLAYERS],
}

impl Image {
    pub fn inventory(&self, player: PlayerId) -> [u8; ITEMS] {
        self.items
            .map(|places| places[Place::Inventory(player).index()])
    }

    pub fn remove_player(&mut self, player: PlayerId) {
        self.players[player.0] = None;
        for places in &mut self.items {
            places[Place::Inventory(player).index()] = 0;
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        generation: Generation,
        inventory: [u8; ITEMS],
    ) {
        self.remove_player(player);
        self.players[player.0] = Some(Actor { generation });
        for (item, count) in inventory.into_iter().enumerate() {
            self.items[item][Place::Inventory(player).index()] = count;
        }
    }

    pub fn difference(&self, other: &Self) -> u64 {
        self.items
            .iter()
            .zip(other.items)
            .filter(|(a, b)| **a != *b)
            .count() as u64
            + self
                .players
                .iter()
                .zip(other.players)
                .filter(|(a, b)| **a != *b)
                .count() as u64
    }
}

#[derive(Clone, Debug)]
pub enum Delta {
    Item {
        item: ItemId,
        places: [u8; PLAYERS + 2],
    },
    Player {
        player: PlayerId,
        actor: Option<Actor>,
    },
}

impl Delta {
    pub fn between(before: Image, after: Image) -> Vec<Self> {
        let mut changes = Vec::new();
        for item in 0..ITEMS {
            if before.items[item] != after.items[item] {
                changes.push(Self::Item {
                    item: ItemId(item),
                    places: after.items[item],
                });
            }
        }
        for player in 0..PLAYERS {
            if before.players[player] != after.players[player] {
                changes.push(Self::Player {
                    player: PlayerId(player),
                    actor: after.players[player],
                });
            }
        }
        changes
    }

    pub(crate) fn apply(&self, image: &mut Image) {
        match *self {
            Self::Item { item, places } => image.items[item.0] = places,
            Self::Player { player, actor } => image.players[player.0] = actor,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub fence: Fence,
    pub sequence: u64,
    pub image: Image,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub live: Image,
    pub cut_log: crate::CutLog,
    durable_head: Image,
    checkpoint: Image,
    checkpoint_sequence: u64,
    sequence: u64,
    journal: Vec<(u64, Vec<Delta>)>,
}

impl Tile {
    pub fn new(image: Image) -> Self {
        Self {
            live: image,
            cut_log: crate::CutLog::new(image),
            durable_head: image,
            checkpoint: image,
            checkpoint_sequence: 0,
            sequence: 0,
            journal: Vec::new(),
        }
    }

    pub fn prepare_checkpoint(&self, fence: Fence) -> Checkpoint {
        Checkpoint {
            fence,
            sequence: self.sequence,
            image: self.live,
        }
    }

    pub fn prepare_logged_checkpoint(&self, fence: Fence) -> Checkpoint {
        // Log deltas are relative to this head. A checkpoint of unlogged live
        // lifecycle changes would invalidate their base if it landed later.
        Checkpoint {
            fence,
            sequence: self.sequence,
            image: self.durable_head,
        }
    }

    pub fn install_checkpoint(&mut self, checkpoint: &Checkpoint, current_epoch: Epoch) -> bool {
        if checkpoint.fence.epoch != current_epoch || checkpoint.sequence < self.checkpoint_sequence
        {
            return false;
        }
        self.checkpoint = checkpoint.image;
        self.checkpoint_sequence = checkpoint.sequence;
        self.journal
            .retain(|(sequence, _)| *sequence > checkpoint.sequence);
        self.durable_head = self.replayed();
        true
    }

    pub fn change(&mut self, image: Image) {
        self.sequence += 1;
        self.live = image;
    }

    pub fn baseline_save_player(
        &mut self,
        player: PlayerId,
        generation: Generation,
        inventory: [u8; ITEMS],
    ) {
        self.checkpoint.insert_player(player, generation, inventory);
    }

    /// One atomic durable tick batch. An adapter must fsync before this input.
    pub fn durable_change(&mut self, image: Image) {
        self.commit_delta(Delta::between(self.live, image));
    }

    pub fn commit_delta(&mut self, changes: Vec<Delta>) {
        self.sequence += 1;
        for delta in &changes {
            delta.apply(&mut self.live);
        }
        // Recovery can materialize a committed entry from the shared store.
        // Include that baseline before any dependent tick, even if its actor
        // was unchanged by the input. Diffing only live-before/live-after loses it.
        self.journal
            .push((self.sequence, Delta::between(self.durable_head, self.live)));
        self.durable_head = self.live;
    }

    pub fn recover(&mut self) -> u64 {
        let before = self.live;
        self.live = self.replayed();
        self.durable_head = self.live;
        // Never reuse sequence numbers after restoring an earlier image.
        before.difference(&self.live)
    }

    fn replayed(&self) -> Image {
        let mut image = self.checkpoint;
        for (_, batch) in &self.journal {
            for delta in batch {
                delta.apply(&mut image);
            }
        }
        image
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TileGeometry {
    edge: i32,
}

impl TileGeometry {
    pub fn new(edge: i32) -> Option<Self> {
        (edge > 0).then_some(Self { edge })
    }

    pub fn tile_at(self, x: i32, z: i32) -> (i32, i32) {
        (x.div_euclid(self.edge), z.div_euclid(self.edge))
    }

    pub fn neighbours(a: (i32, i32), b: (i32, i32)) -> bool {
        a.0.abs_diff(b.0) as u64 + a.1.abs_diff(b.1) as u64 == 1
    }
}
