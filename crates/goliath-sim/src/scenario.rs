//! Owns reproducible bot workloads and explicit fault schedules.

use goliath_model::{ItemId, PlayerId, TileId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Take,
    Drop,
    PickUp,
    Store,
    Cross(TileId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Link {
    TileStore,
    GatewayTile,
    TileTile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    Crash(TileId),
    Partition(Link),
    StoreDown,
    NetworkChaos,
    Heal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaultAt {
    pub at: u64,
    pub fault: Fault,
}

#[derive(Clone, Copy, Debug)]
pub struct Latency {
    pub checkpoint_ms: u64,
    pub fsync_ms: u64,
    pub checkpoint_interval_ms: u64,
    pub network_ms: u64,
}

pub const SETUPS: [Latency; 3] = [
    Latency {
        checkpoint_ms: 10,
        fsync_ms: 2,
        checkpoint_interval_ms: 100,
        network_ms: 3,
    },
    Latency {
        checkpoint_ms: 80,
        fsync_ms: 8,
        checkpoint_interval_ms: 200,
        network_ms: 3,
    },
    Latency {
        checkpoint_ms: 250,
        fsync_ms: 20,
        checkpoint_interval_ms: 400,
        network_ms: 3,
    },
];

#[derive(Clone, Debug)]
pub struct Scenario {
    pub seed: u64,
    pub latency: Latency,
    pub faults: Vec<FaultAt>,
    pub inputs: Vec<(u64, PlayerId, Action, ItemId)>,
}

pub(crate) fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

impl Scenario {
    pub fn seeded(seed: u64, latency: Latency) -> Self {
        let mut rng = mix(seed);
        let mut next = || {
            rng = mix(rng);
            rng
        };
        let mut inputs = vec![
            (0, PlayerId(1), Action::Take, ItemId(0)),
            (2, PlayerId(0), Action::Drop, ItemId(4)),
            (4, PlayerId(1), Action::PickUp, ItemId(4)),
            (6, PlayerId(1), Action::Cross(TileId(1)), ItemId(0)),
            (100, PlayerId(1), Action::Store, ItemId(0)),
            (110, PlayerId(1), Action::Drop, ItemId(4)),
            (120, PlayerId(2), Action::PickUp, ItemId(4)),
            (130, PlayerId(2), Action::Cross(TileId(2)), ItemId(4)),
            (180, PlayerId(2), Action::Store, ItemId(4)),
        ];
        for _ in 0..28 {
            let at = 10 + next() % 580;
            let player = PlayerId((next() % 3) as usize);
            let item = ItemId((next() % 8) as usize);
            let action = match next() % 5 {
                0 => Action::Take,
                1 => Action::Drop,
                2 => Action::PickUp,
                3 => Action::Store,
                _ => Action::Cross(TileId((next() % 3) as usize)),
            };
            inputs.push((at, player, action, item));
        }
        let mut faults = vec![FaultAt {
            at: 8 + next() % 330,
            fault: Fault::Crash(TileId(0)),
        }];
        for _ in 0..4 {
            let at = next() % 650;
            let fault = match next() % 7 {
                0..=2 => Fault::Crash(TileId((next() % 3) as usize)),
                3 => Fault::Partition(Link::TileStore),
                4 => Fault::Partition(Link::GatewayTile),
                5 => Fault::Partition(Link::TileTile),
                _ => Fault::StoreDown,
            };
            faults.push(FaultAt { at, fault });
        }
        faults.push(FaultAt {
            at: next() % 400,
            fault: Fault::NetworkChaos,
        });
        faults.push(FaultAt {
            at: 450 + next() % 200,
            fault: Fault::Heal,
        });
        faults.sort_by_key(|f| f.at);
        inputs.sort_by_key(|input| input.0);
        Self {
            seed,
            latency,
            faults,
            inputs,
        }
    }
}
