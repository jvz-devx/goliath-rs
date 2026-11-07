//! Owns handover authority, crash recovery and conservation checks.

use crate::authority::Record;
use crate::{
    Directory, Fence, Generation, ITEMS, Image, PLAYERS, Place, PlayerId, Store, TILES, Tile,
    TileId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy {
    P0,
    P1,
    P2,
    P3,
    P4,
}

impl Policy {
    pub const ALL: [Self; 5] = [Self::P0, Self::P1, Self::P2, Self::P3, Self::P4];
    pub fn name(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Handover {
    pub player: PlayerId,
    pub source: TileId,
    pub target: TileId,
    pub generation: Generation,
    pub inventory: [u8; ITEMS],
    pub source_fence: Fence,
    pub target_fence: Fence,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub tiles: [Tile; TILES],
    pub directory: Directory,
    pub store: Store,
    pub policy: Policy,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Audit {
    pub counts: [u64; ITEMS],
    pub locations: [[u64; TILES * 2 + PLAYERS]; ITEMS],
    pub player_locations: [[u8; TILES]; PLAYERS],
    pub duplicate_identities: u64,
    pub missing_identities: u64,
    pub player_owner_violations: u64,
    pub stale_writes: u64,
}

impl Audit {
    pub fn violated(&self) -> bool {
        self.duplicate_identities + self.player_owner_violations + self.stale_writes > 0
    }

    /// Changed authoritative item-location rows plus changed player-owner rows.
    /// Escrow to resident materialization and stale replica cleanup are not rollback.
    pub fn difference(&self, other: &Self) -> u64 {
        self.locations
            .iter()
            .zip(other.locations)
            .filter(|(a, b)| **a != *b)
            .count() as u64
            + self
                .player_locations
                .iter()
                .zip(other.player_locations)
                .filter(|(a, b)| **a != *b)
                .count() as u64
    }
}

impl Cluster {
    pub fn new(policy: Policy) -> Self {
        let mut images = [Image::default(); TILES];
        images[0].players[0] = Some(crate::Actor {
            generation: Generation(0),
        });
        images[0].players[1] = Some(crate::Actor {
            generation: Generation(0),
        });
        images[1].players[2] = Some(crate::Actor {
            generation: Generation(0),
        });
        for item in 0..4 {
            images[0].items[item][Place::Chest.index()] = 1;
        }
        images[0].items[4][Place::Inventory(PlayerId(0)).index()] = 1;
        images[0].items[5][Place::Inventory(PlayerId(0)).index()] = 1;
        images[1].items[6][Place::Inventory(PlayerId(2)).index()] = 1;
        images[1].items[7][Place::Chest.index()] = 1;
        let records = std::array::from_fn(|p| {
            let owner = TileId(usize::from(p == 2));
            Record {
                generation: Generation(0),
                owner,
                inventory: images[owner.0].inventory(PlayerId(p)),
            }
        });
        Self {
            tiles: images.map(Tile::new),
            directory: Directory::default(),
            store: Store {
                records,
                rejected: 0,
                stale_accepted: 0,
            },
            policy,
        }
    }

    pub fn actor_tile(&self, player: PlayerId) -> Option<TileId> {
        self.tiles.iter().enumerate().find_map(|(t, tile)| {
            tile.live.players[player.0].and_then(|actor| {
                let record = self.store.records[player.0];
                (self.policy == Policy::P0
                    || (record.owner == TileId(t) && record.generation == actor.generation))
                    .then_some(TileId(t))
            })
        })
    }

    pub fn handover(&self, player: PlayerId, target: TileId) -> Option<Handover> {
        let source = self.actor_tile(player)?;
        if source.0.abs_diff(target.0) != 1 {
            return None;
        }
        let actor = self.tiles[source.0].live.players[player.0]?;
        Some(Handover {
            player,
            source,
            target,
            generation: actor.generation,
            inventory: self.tiles[source.0].live.inventory(player),
            source_fence: self.directory.current(source),
            target_fence: self.directory.current(target),
        })
    }

    pub fn arrive(&mut self, handover: Handover) {
        // Retry must not reset an inventory which the target has already changed.
        if self.tiles[handover.target.0].live.players[handover.player.0]
            .is_some_and(|actor| actor.generation == Generation(handover.generation.0 + 1))
        {
            return;
        }
        let mut image = self.tiles[handover.target.0].live;
        image.insert_player(
            handover.player,
            Generation(handover.generation.0 + 1),
            handover.inventory,
        );
        if self.policy == Policy::P4 {
            self.tiles[handover.target.0].durable_change(image);
        } else {
            self.tiles[handover.target.0].change(image);
        }
    }

    pub fn release(&mut self, handover: Handover) {
        if self.tiles[handover.source.0].live.players[handover.player.0]
            .is_some_and(|actor| actor.generation == handover.generation)
        {
            let mut image = self.tiles[handover.source.0].live;
            image.remove_player(handover.player);
            self.tiles[handover.source.0].change(image);
        }
    }

    pub fn recover(&mut self, tile: TileId) -> u64 {
        let before = self.tiles[tile.0].live;
        self.crash(tile);
        self.reconcile(tile);
        before.difference(&self.tiles[tile.0].live)
    }

    /// Lose volatile state and replace the process. Store access is a later input.
    pub fn crash(&mut self, tile: TileId) {
        self.directory.replace(tile);
        self.tiles[tile.0].recover();
    }

    /// Called only after the adapter can read the shared directory/store.
    pub fn reconcile(&mut self, tile: TileId) {
        if self.policy != Policy::P0 {
            for p in 0..PLAYERS {
                let player = PlayerId(p);
                let record = self.store.records[p];
                let image = &mut self.tiles[tile.0].live;
                if record.owner != tile {
                    image.remove_player(player);
                } else if image.players[p].is_none_or(|actor| actor.generation != record.generation)
                {
                    image.insert_player(player, record.generation, record.inventory);
                }
            }
        }
    }

    pub fn audit(&self) -> Audit {
        let mut result = Audit::default();
        let mut owners = [0_u8; PLAYERS];
        for (t, tile) in self.tiles.iter().enumerate() {
            for (p, actor) in tile.live.players.iter().enumerate() {
                if let Some(actor) = actor {
                    let record = self.store.records[p];
                    if self.policy == Policy::P0
                        || (record.owner == TileId(t) && record.generation == actor.generation)
                    {
                        owners[p] += 1;
                        result.player_locations[p][t] += 1;
                    }
                }
            }
            for (item, places) in tile.live.items.iter().enumerate() {
                result.locations[item][t * 2] += u64::from(places[0]);
                result.locations[item][t * 2 + 1] += u64::from(places[1]);
                for p in 0..PLAYERS {
                    if tile.live.players[p].is_some_and(|actor| {
                        let record = self.store.records[p];
                        self.policy == Policy::P0
                            || (record.owner == TileId(t) && record.generation == actor.generation)
                    }) {
                        result.locations[item][TILES * 2 + p] += u64::from(places[p + 2]);
                    }
                }
            }
        }
        if self.policy != Policy::P0 {
            // A committed record is an escrow until routing or recovery installs it.
            for (p, count) in owners.iter_mut().enumerate() {
                if *count == 0 {
                    *count = 1;
                    result.player_locations[p][self.store.records[p].owner.0] = 1;
                    for (i, count) in self.store.records[p].inventory.iter().enumerate() {
                        result.locations[i][TILES * 2 + p] += u64::from(*count);
                    }
                }
            }
        }
        result.counts = result.locations.map(|places| places.into_iter().sum());
        result.duplicate_identities = result.counts.iter().filter(|n| **n > 1).count() as u64;
        result.missing_identities = result.counts.iter().filter(|n| **n == 0).count() as u64;
        result.player_owner_violations = owners.iter().filter(|n| **n > 1).count() as u64;
        result.stale_writes = self.store.stale_accepted;
        result
    }
}
