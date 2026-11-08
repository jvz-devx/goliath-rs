# Experiment patches

Apply patches from the matching upstream checkout. Nothing here has been sent upstream.

- `horizon-01-tile-cut.patch`: E6 opt-in tick-boundary persistent-state capture,
  container mutation tracking, a checksummed cut log, grouped full flush and recovery.
  Base is Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92` after
  `e2-horizon-input-trace.patch`. Apply E2 first, then this patch.
  `HORIZON_E3_CUT_DIR` and `HORIZON_E3_FENCE` enable it. The ordinary save and
  control behavior are unchanged without these variables. `tile-fence` claims
  an opaque process token with CAS; `tile-cut <token> [departing-uuid]` captures one cut.
  With a UUID it runs the existing export/freeze and captures at the same boundary.
  See `docs/experiments/E6-fast-cut.md` for timings and limits. This is an
  experiment, not a production replacement for Horizon's save system.
  A cut is refused when a live entity or container lies outside resident chunks.
  The first cut still uses E3's full-save bootstrap. Warm cuts never wait on its
  region writers. Chunk tick metadata is separate from terrain and containers.
  The control prelude also runs during tick pacing. Idle control work counts in MSPT.
  Unloads, changed player sets and newly accepted generation/proto-save state
  require a restart and a new full base. Pending entity spawns refuse before
  freezing and may be retried after a tick. Light jobs alone do not block cuts.
  Native mods are refused. Use a fresh cut directory, not an E3 HEAD archive.
  Complete BASE and LOG records, not a HEAD file, select recovery.

- `meridian-01-wall-trigger.patch`: E4 static tile configuration, position-triggered serialized
  handoffs, fixed crossing-position arrival, geometry tests and the command-free `e4-client`.
  Base is Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d` after
  `e2-meridian-client.patch`. Run `git apply --check` before `git apply`.
  The trigger has a 0.5-block margin on both axes and coalesces in-flight movement to the
  latest destination. It adds no Horizon ownership or durability changes.
  Optional `[tiles.wall-particles]` drawing is off by default. It supports blue RGB dust
  or end rods, configurable spacing and distance, a local patch, and at most two bursts
  per second. Disabled sessions retain the original routing hook with no drawing work.

E2 patch provenance, application order and MIT notices are in [README-E2.md](README-E2.md).

- `horizon-02-tile-ownership.patch`: E7 opt-in static tile ticking, solid unknown
  reads, collision and entity containment, generation filtering and Anvil write
  refusal. Base is Horizon `fc2868b2b` after `e2-horizon-input-trace.patch`.
  `HORIZON_TILE=origin_x,origin_z,edge,index_x,index_z` enables it. Origin and edge
  must align to 512 blocks. `HORIZON_TILE_TEST_SMALL=1` permits 16-block alignment
  only for experiments. The same mask applies in every dimension. Exterior client
  views are not loaded. The outer two owned chunk rings skip decoration.
  `HORIZON_E7_READ_PROBE=1` logs seven simulation reads before chunk loading.
  See `docs/experiments/E7-tile-ownership.md` for evidence and limits.
No Folia, Paper, other GPL source or `docs/research/05-folia.md` was read for E4.

- `meridian-02-fast-crossing.patch`: E8 refusal recovery after death, expiring
  approach preparation, explicit buffered arrival-ring checks, bounded ordered
  input replay, lane-bound keep-alive, ping, teleport and block-sequence mapping,
  and a continuous same-world arrival using Horizon's existing durable
  `player-import` placement mode. Apply after E2 and `meridian-01-wall-trigger.patch`
  on Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`.
  `E8_STAGE=1`, `2`, `3`, `4` selects cumulative experiment configurations.
  The default is 4. Ordinary command switches keep their old settings.
  Stage 4 acknowledges both target corrections on the passive lane and omits
  the synthetic client teleport. It carries the exported server velocity and
  replays input after durable target placement, before source release.
  This does not implement the D6 coherent checkpoint, store fence, terrain
  composition or crash recovery. See `docs/experiments/E8-fast-crossing.md`.
  `python3 -B scripts/e8-patch` rebuilds the delta mechanically, checks the
  application order in an isolated archive, and compares resulting source bytes.
  No Folia, Paper, other GPL source or excluded research was read for E8.

- `horizon-03-bots.patch`: E11 opt-in single-threaded `horizon-bots --event-loop`,
  seeded spread routes, explicit wall-crossing paths, configuration re-entry,
  Respawn cache reset and Transfer reconnects. Base is Horizon `fc2868b2b`
  after `e2-horizon-input-trace.patch`. Existing bot modes stay unchanged.
  Mio is already in the pinned lockfile. See `docs/experiments/E11-measurement.md`.

- `horizon-04-digest.patch`: E11 opt-in fixture-region SHA-256 digests,
  bounded individual tick samples and mob census. Same base plus E2.
  `HORIZON_E11_MEASURE=1` enables control commands `e11-digest` and
  `e11-samples`. The digest covers blocks, live block-entity save NBT and
  entity kind/position multisets, not RNG, lighting or full entity state.
  It rejects unloaded regions and unsupported entity kinds. Both E11 patches
  can be applied with E7. `scripts/e11-build` retains plain and E7 release
  engines, with ThinLTO and one codegen unit. No GPL source or excluded
  research was read. Nothing has been sent upstream.

- `horizon-03-cluster.patch`: E10 delta after E2, E6's
  `horizon-01-tile-cut.patch` and E7's `horizon-02-tile-ownership.patch`, on
  Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. Tracks unloads,
  departures and accepted generation in cuts. Adds full-UUID store registration,
  exact committed-blob activation checks, generation-aware recovery, positioned
  held logins and control queue timing. Cluster movement stays server-clamped
  without sending the wall correction that prevented a proxy crossing.
  `GOLIATH_STORE`, `GOLIATH_TILE_ID` and the store-derived `HORIZON_E3_FENCE`
  enable authority integration. Ordinary E6/E7 behavior stays opt-in separately.
  The path dependency expects this repository's `crates/goliath-store` in the
  normal `.local/src/horizon` layout. See `docs/experiments/E10-cluster.md`.

- `meridian-03-cluster.patch`: E10 delta after E2, E4's
  `meridian-01-wall-trigger.patch` and E8's `meridian-02-fast-crossing.patch`,
  on Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`. Adds the path
  dependency on `goliath-store`, source cut then fenced CAS then target
  activation, store-owned reconnect routing and cluster sky packets.
  `GOLIATH_CLUSTER` selects the private runtime manifest; `GOLIATH_CLOCK`
  supplies the store's startup sky anchor. It does not implement proxy chat,
  a global tab list, scoreboard transfer or neighboring terrain composition.
  `scripts/e10-patch` recreates both E10 deltas and checks clean application
  plus exact source equality. The final check includes the test client's
  corrected 26.3 drop action and the persistence wrapper's proto-snapshot
  forwarding. Both deltas apply and reproduce the working source byte for byte.

- `horizon-05-m2.patch`: E12 after the complete E10 Horizon stack on
  `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. Sends owned edge columns without
  waiting for foreign light neighbours. Adds held, non-placeable terrain readers,
  immutable cut pre-encoding, single-player vehicle groups, activation input and
  full live player-state transfer. The latter preserves transient attribute
  modifiers, controls, abilities, shared flags, pose and cooldown progress.
  It does not change client physics or relax the lit-column guard.

- `meridian-04-m2.patch`: E12 after the complete E10 Meridian stack on
  `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`. Composes terrain by fixed column
  owner, filters observer entities, acknowledges suppressed active-lane chunk
  batches and keeps terrain across a swap. Server placements drive observers
  even without client movement. The delta takes authoritative live attributes
  rather than held-login defaults, and own metadata is coalesced by field.

- `horizon-06-leases.patch`: E12's E16 integration after `horizon-05-m2.patch`.
  Requires the E16 store/model API. Horizon itself claims an original-send lease,
  owns its renewal thread and exposes the actual fence through `cluster-authority`.
  Keeper locks never span network IO. A caller-supplied core gate protects login,
  placement, packet dispatch, append, flush, installation and recovery writes.
  Expiry latches the process read-only; healing does not revive it. Monotonic
  clock comparison detects suspension without using wall time for authority.
  Critical IO reserves 100 ms and checks again after barriers. This is not a
  physical storage fence against unbounded pauses or IO inside a syscall.

- `horizon-07-observer-status.patch`: after `horizon-06-leases.patch`. Adds
  exact placed/held/terrain_reader guest status, frozen/released and chunk/wire
  counters, and an observer-only radius request. `player-observe UUID` accepts
  either the old position array or `{"position":[x,y,z],"radius":8}`. Explicit
  radii 2 through 32 control loading and tracking without changing real-player
  view caps. It also contains the final post-IO recovery authority checks.

The canonical E17 handoff is `.local/bin/goliath-e12/FINAL.json`, with ordered
patch SHA-256 values, internal-crate source identities, store identity and
scalable transfer admission settings. Its Meridian entry is frozen at E12's
handover. E18 owns successor Meridian patches. E12 no longer regenerates that
source from E18's worktree.

E12 uses `.local/bin/goliath-e12/LEASES` to select leased launcher setup. That
setup skips legacy `Claim`, starts Horizon with an explicit takeover permission,
then reads its granted fence instead of inventing a local deadline. Never mix
leased Horizon with the retained E10 store. The E16 store is retained separately
and copied by `scripts/e12-m2 build`. Old unleased experiments remain the default
when their binary directory has no `LEASES` marker. `GOLIATH_BIN_DIR` selects a
binary directory for the shared runner and E13 fixtures without changing their
default E10 evidence. Keep the marker when making a leased binary snapshot.

`python3 -B scripts/e12-patch` applies the pinned stacks in disposable archives
and checks exact working-source byte equality, including the separate lease
delta. E12's report and permanent packet checks are in
`docs/experiments/E12-m2.md`. No E12 patch has been committed or sent upstream.
