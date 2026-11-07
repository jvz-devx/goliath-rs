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
  Unloads, changed player sets and incomplete streaming fail closed until restart.
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
