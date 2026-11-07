//! Owns authority, conservation and persistence regression tests.

use goliath_model::{
    Cluster, Epoch, Generation, Place, PlayerId, Policy, StoreError, TileGeometry, TileId,
};

#[test]
fn negative_coordinates_use_floor_division() {
    let geometry = TileGeometry::new(16).unwrap();
    assert_eq!(geometry.tile_at(-1, -16), (-1, -1));
    assert_eq!(geometry.tile_at(-17, 16), (-2, 1));
    assert!(TileGeometry::new(0).is_none());
    assert!(TileGeometry::neighbours((-1, 0), (0, 0)));
    assert!(!TileGeometry::neighbours((-1, 0), (0, 1)));
}

#[test]
fn cas_is_idempotent_and_old_epoch_cannot_write() {
    let mut c = Cluster::new(Policy::P1);
    let transfer = c.handover(PlayerId(0), TileId(1)).unwrap();
    assert_eq!(c.store.commit(&c.directory, &transfer), Ok(true));
    assert_eq!(c.store.commit(&c.directory, &transfer), Ok(false));
    let mut conflicting = transfer;
    conflicting.generation = Generation(42);
    assert_eq!(
        c.store.commit(&c.directory, &conflicting),
        Err(StoreError::GenerationConflict)
    );
    c.directory.replace(TileId(0));
    assert_eq!(
        c.store.commit(&c.directory, &transfer),
        Err(StoreError::StaleFence)
    );
    assert_eq!(c.store.records[0].generation, Generation(1));
}

#[test]
fn planted_disabled_fence_is_caught_by_independent_checker() {
    let mut c = Cluster::new(Policy::P1);
    let transfer = c.handover(PlayerId(0), TileId(1)).unwrap();
    c.directory.replace(TileId(0));
    c.directory.fencing = false;
    assert_eq!(c.store.commit(&c.directory, &transfer), Ok(true));
    assert!(c.audit().violated());
    assert_eq!(c.audit().stale_writes, 1);
}

#[test]
fn checkpoint_is_atomic_old_epoch_and_old_sequence_are_rejected() {
    let mut c = Cluster::new(Policy::P2);
    let old = c.tiles[0].prepare_checkpoint(c.directory.current(TileId(0)));
    let mut after = c.tiles[0].live;
    after.items[0][0] = 0;
    after.items[0][2] = 1;
    c.tiles[0].change(after);
    let fresh = c.tiles[0].prepare_checkpoint(c.directory.current(TileId(0)));
    assert!(c.tiles[0].install_checkpoint(&fresh, Epoch(0)));
    assert!(!c.tiles[0].install_checkpoint(&old, Epoch(0)));
    assert!(!c.tiles[0].install_checkpoint(&fresh, Epoch(1)));
    c.tiles[0].recover();
    assert_eq!(c.tiles[0].live, after);
}

#[test]
fn checkpoint_plus_later_log_batch_recovers_both_changes() {
    let mut c = Cluster::new(Policy::P4);
    let mut after = c.tiles[0].live;
    after.items[0][0] = 0;
    after.items[0][2] = 1;
    c.tiles[0].durable_change(after);
    let checkpoint = c.tiles[0].prepare_checkpoint(c.directory.current(TileId(0)));
    after.items[0][2] = 0;
    after.items[0][1] = 1;
    c.tiles[0].durable_change(after);
    assert!(c.tiles[0].install_checkpoint(&checkpoint, Epoch(0)));
    c.tiles[0].recover();
    assert_eq!(c.tiles[0].live, after);
}

#[test]
fn two_copies_inside_one_tile_are_not_overwritten_or_hidden() {
    let mut c = Cluster::new(Policy::P1);
    c.tiles[0].live.items[0][Place::Inventory(PlayerId(0)).index()] = 1;
    assert_eq!(c.audit().counts[0], 2);
    assert_eq!(c.audit().duplicate_identities, 1);
}

#[test]
fn coherent_checkpoint_covers_transitive_donor_inventory() {
    for policy in [Policy::P1, Policy::P2] {
        let mut c = Cluster::new(policy);
        let mut image = c.tiles[0].live;
        image.items[4][Place::Inventory(PlayerId(0)).index()] = 0;
        image.items[4][Place::Inventory(PlayerId(1)).index()] = 1;
        c.tiles[0].change(image);
        if policy == Policy::P2 {
            let checkpoint = c.tiles[0].prepare_checkpoint(c.directory.current(TileId(0)));
            c.tiles[0].install_checkpoint(&checkpoint, Epoch(0));
        }
        let transfer = c.handover(PlayerId(1), TileId(1)).unwrap();
        c.store.commit(&c.directory, &transfer).unwrap();
        c.arrive(transfer);
        c.release(transfer);
        c.recover(TileId(0));
        assert_eq!(c.audit().duplicate_identities > 0, policy == Policy::P1);
    }
}

#[test]
fn wal_anchors_store_materialization_before_dependent_deposit() {
    let mut c = Cluster::new(Policy::P4);
    let transfer = c.handover(PlayerId(0), TileId(1)).unwrap();
    c.store.commit(&c.directory, &transfer).unwrap();
    // Target dies before its arrival WAL. Recovery materializes the shared entry.
    c.recover(TileId(1));
    let mut image = c.tiles[1].live;
    image.items[4][Place::Inventory(PlayerId(0)).index()] = 0;
    image.items[4][Place::Chest.index()] = 1;
    c.tiles[1].durable_change(image);
    c.recover(TileId(1));
    assert_eq!(c.audit().counts, [1; 8]);
}

#[test]
fn log_checkpoint_base_does_not_include_unlogged_departure() {
    let mut c = Cluster::new(Policy::P4);
    let original = c.tiles[0].live;
    let mut absent = original;
    absent.remove_player(PlayerId(0));
    c.tiles[0].change(absent);
    let checkpoint = c.tiles[0].prepare_logged_checkpoint(c.directory.current(TileId(0)));
    let mut returned = original;
    returned.players[0].as_mut().unwrap().generation = Generation(2);
    c.tiles[0].durable_change(returned);
    c.tiles[0].install_checkpoint(&checkpoint, Epoch(0));
    c.tiles[0].recover();
    assert_eq!(c.tiles[0].live, returned);
}
