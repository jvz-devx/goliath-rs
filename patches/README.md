# Experiment patches

Apply patches from the matching upstream checkout. Nothing here has been sent upstream.

The accepted measurement stack is FINAL-09 tiles, engine SHA-256 beginning
`b2f1ea65`, with the reactor-1 proxy. It supports the reproduced 650-player
frontier, not production acceptance or the 3x target. See
[E20](../docs/experiments/E20-frontier.md) and
[E23's frozen identities](../docs/experiments/E23-closeout.md#frozen-identities-and-reproduction).
The root [stack inventory](../README.md#current-experiment-stacks) gives exact
pins and patch order. Entries below retain earlier checks and failures as history.
Patches 10 and 11, scheduler proxies and shared views do not replace that stack.
E22's shared views remain on hold, unqualified.

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
`docs/experiments/E12-m2.md`. These are retained local experiment deltas, not
upstream changes.

- `meridian-05-proxy-cost.patch`: E18 after the immutable E12 Meridian stack,
  ending at `meridian-04-m2.patch`, on
  `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`. Pools authenticated Horizon
  control connections without replaying uncertain commands. Definite full-cut-
  queue refusals defer wall crossings with backoff. Adds bounded read-drain
  write coalescing without a timer. `E18_CONTROL_FRESH=1` and
  `E18_WRITES_DIRECT=1` select transport ablations. Store authority and leases
  are unchanged. `python3 -B scripts/e18-patch` verifies the frozen prerequisite
  patch hashes, clean application and complete source byte equality. This patch
  belongs to the selected reactor-1 stack with patch 06. It is not a standalone
  M2 or M4 acceptance claim. See
  `docs/experiments/E18-proxy-cost.md`.

- `horizon-09-flat-light.patch`: E18's local patch for an upstream flat-world
  light-view retry loop at `fc2868b2b26d56ec76e17414909fe58a3b87bb92`.
  `FlatTerrain` already stores a full `Chunk` as its proto. It now provides a
  cloned read-only light view, so normal lighting can finish instead of repeatedly
  receiving `None`. No pacing, condvar, lease or ownership rule changes.
  Apply this exact patch to both the W1-equivalent plain stack and FINAL tile
  stack before comparing capacity. E17 repeated the staircase with this patch:
  the matched baseline passes 325 twice and fails 350 twice. E20's selected
  cluster passes 650 twice; the 975-player target is not met.
  A resource-free unit test covers the 3 by 3 views and COW immutability. The
  `flat_light` streaming regression requires prepared resources through
  `HORIZON_TEST_RESOURCES` or the standard local resource directory; it never
  silently skips missing resources. Native release red/green builds are recorded
  by `scripts/e18-flat-build`. Patch application and exact source equality are in
  `docs/experiments/data/E18/flat09-patch-proof.json`. This is an upstream issue
  patched locally. Nothing has been sent upstream or to GitHub.

- `meridian-06-io-pool.patch`: E18's experimental event-loop delta after
  frozen patch 05. The runtime-free `meridian-net::io_pool` transport gives the
  application caller-owned readiness drivers, bounded per-connection FIFO
  output, actual-write completion callbacks and nonblocking partial-frame IO.
  Mio 1.2.4 supplies portable readiness notifications. No Horizon, store,
  authority or lease code changes. Four caller-owned IO drivers share all of a
  session's sockets; four bounded workers isolate command hooks. Pending input
  and output are bounded, and graceful closes wait for queued output. Per-player
  management/supervisor threads remain. Scoped strict release gates pass,
  including encrypted/compressed loopback forwarding, command isolation and
  routing-gate replay. The separate `reactor-1` candidate passed E10's 1,000
  crossings and item/restart fixtures, E12's terrain, border and live-state
  regressions, and the unchanged real-physics checker over 112 crossings.
  One physics timing window remains unresolved and is retained. At 975 players
  under the shared four-core cap it used 1.495 proxy cores and 1,992 threads,
  but failed capacity at 7.23 to 7.49 tile TPS and internal clientbound P95
  20.30 ms with limiter stops. This is not an M4 acceptance or production
  release. Full evidence and residual risks are in `docs/experiments/E18-proxy-cost.md`.
  `python3 -B scripts/e18-patch --event-loop` verifies the frozen stack including
  patch 05, clean application and every source byte without rewriting patch 05.

- E26's diagnostic stack is `horizon-14-tick-timers.patch`, then
  `horizon-15-tick-timer-conditions.patch`, `horizon-16-reader-inbox-timers.patch`,
  `horizon-17-tick-busy-waits.patch` and `horizon-18-tick-wait-detail.patch`.
  Apply it after FINAL-09 and the recorded E11 measurement stack, not after
  the unselected warm-tick or shared-observer candidates. It records exclusive
  main-thread costs, with reader-only delivery and inbox scopes. Explicit
  pacing sleep and worker-completion waiting are recorded separately from
  busy work. `e26-timers on` enables only the main thread serving that control
  endpoint; `e26-timers drain` returns bounded per-tick rows and dropped counts.
  A separately named build with `HORIZON_E26_PLANTED_COST=1` adds a 200 us
  pulse once per dedicated tick for the positive control. These are diagnostic
  builds, not selected production or M4 acceptance binaries. Native gates and
  measurements are tracked in `docs/experiments/E26-tick-profile.md`.

- `horizon-21-lite-cpu-clean.patch`: the clean, measurement-only two-kind
  main-thread CPU probe used by E26b. It is the exact five-file net effect
  of patches 19 and 20, without full-timer hooks. It applies after the
  FINAL-09 measurement stack and also after horizon-13 renewal-r2 plus
  patch 10. The separately frozen lite bundle is not the untimed runtime
  candidate. Final-stack load runs belong to the reviewer; see the E26
  report for complete bundles, hashes and the guarded lite-B command.

- `horizon-08-m4-measurement.patch`: E17's measurement-only integration on
  Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. Apply the E12 FINAL stack
  through `horizon-07-observer-status.patch`, then `horizon-03-bots.patch`,
  then `horizon-04-digest.patch` with
  `--exclude=crates/horizon-client/src/game/dedicated/status.rs`, then this patch.
  It wires `e11-digest`, `e11-samples` and population status into the leased
  control endpoint. The digest patch's other files supply their implementations.
  The release prerequisite export and startup/control smoke passed. This is
  instrumentation, not a capacity qualification by itself. It remains in
  FINAL-09 before patch 09. See [E17 release provenance](../docs/experiments/E17-m4.md#release-provenance)
  and the [ordered manifest](../docs/experiments/data/E17/final-release-prereqs-v2.json).

- `horizon-10-warm-tick.patch`: E21's candidate on Horizon
  `fc2868b2b26d56ec76e17414909fe58a3b87bb92`, after the complete FINAL-09
  production and measurement stack, ending at `horizon-09-flat-light.patch`.
  It admits speculative cut warming at most once per world tick. The real cut
  still checks freshness. Release gates and same-tick mutation recovery pass.
  E20's combined scheduler-2/warmtick-2 suite passes checker v2 with three
  unresolved timing windows retained. The two 650-player capacity pilots are
  mixed, so this candidate does not replace the accepted frontier stack.
  See [E21](../docs/experiments/E21-tile-cost.md#patch-10) and
  [E20 combined validation](../docs/experiments/E20-frontier.md#combined-validation-selected-by-the-reviewer).

- `horizon-11-shared-observer.patch`: E22's shared-reader candidate on Horizon
  `fc2868b2b26d56ec76e17414909fe58a3b87bb92`, after
  `horizon-10-warm-tick.patch` and all its prerequisites. It adds bounded sparse
  observer areas, interest revisions, ordered column refresh and status counters.
  Release gates and legacy terrain/border checks pass. Both E22 comparison arms
  use these engine bytes. Shared-4 does not reproduce its CPU-saving rule, and
  its full M2 suite was not run. Hold, unqualified for promotion. See
  [E22](../docs/experiments/E22-shared-views.md#horizon-patch-11).

- `meridian-07-terrain-scheduler.patch`: E20's candidate on Meridian
  `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, after the reactor-1 stack ending
  at `meridian-06-io-pool.patch`. It replaces per-session terrain timers with
  IO-driver timers and bounded shared control workers. Lazy-reader policy and
  authority ordering stay unchanged. Scheduler-1 passes release gates but has
  no separate M2 or capacity qualification. Apply the stop-race follow-up next.
  See [E20](../docs/experiments/E20-frontier.md#shared-scheduler-and-release-verification).

- `meridian-07-scheduler-stop-race.patch`: E20's follow-up on Meridian
  `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, immediately after
  `meridian-07-terrain-scheduler.patch`. It rechecks terminal reconciliation
  state after claiming a worker slot, so completed work cannot restart.
  Scheduler-2 passes release gates. Its combined M2 evidence uses warmtick-2
  and checker v2, with three unresolved timing windows. Mixed capacity pilots
  do not establish a new frontier. It remains a candidate. See
  [E20](../docs/experiments/E20-frontier.md#shared-scheduler-and-release-verification).

- `meridian-08-shared-views.patch`: E22's shared-1 candidate on Meridian
  `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, after both patch-07 files,
  ending at `meridian-07-scheduler-stop-race.patch`. It shares each tile's
  terrain stream through bounded cached snapshots and subscriber interests.
  Shared-1 has a known return-to-interest terrain defect and cannot qualify.
  Follow with the three immutable fixes below, in order. See
  [E22](../docs/experiments/E22-shared-views.md#meridian-patch-08).

- `meridian-08-cache-source-retention.patch`: E22's shared-2 follow-up on
  Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, immediately after
  `meridian-08-shared-views.patch`. It keeps source-sent snapshots and updates
  until the source sends an ordered Forget, even without local subscribers.
  It adds cache bounds and recovery telemetry. Focused tests cover the original
  defect, but refresh recovery still needs the next patch. No independent
  promotion or capacity qualification is claimed. See
  [E22's retained-source fix](../docs/experiments/E22-shared-views.md#cache-lifecycle-regression-and-follow-up).

- `meridian-08-refresh-interest-barrier.patch`: E22's shared-3 follow-up on
  Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, immediately after
  `meridian-08-cache-source-retention.patch`. It paces refresh retries, clears
  interest, waits for ordered Forgets and restores the union with deadlines.
  Release and focused recovery checks pass. Shared-3 still has a client-cache
  eviction defect, so its earlier M2 results cannot qualify shared-4. See
  [E22's recovery barrier](../docs/experiments/E22-shared-views.md#final-recovery-barrier-and-telemetry).

- `meridian-08-client-cache-presence.patch`: E22's shared-4 follow-up on
  Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, immediately after
  `meridian-08-refresh-interest-barrier.patch`. It checks actual client-cache
  presence before suppressing full columns and restores missing owned columns
  with ordered updates. Release gates and the native eviction/return/flight
  probe pass. The two D9 savings are 0.030 and 0.216 core, so the required
  0.10-core saving does not reproduce. Hold, unqualified; no full shared-4 M2.
  See [E22](../docs/experiments/E22-shared-views.md).

## Diagnostics in progress

These E24 overlays are diagnostic only, pending E24's report and review. They
do not replace the accepted measurement stack or qualify an availability fix.

- `horizon-12-lease-diagnostics.patch` and `horizon-12-lease-diagnostics-r2.patch`:
  E24 renewal and clock-failure telemetry on Horizon
  `fc2868b2b26d56ec76e17414909fe58a3b87bb92`, after the complete FINAL-09 stack.
  Select one overlay after patch 09, not after patches 10/11. Revision 2 replaces
  the first overlay; do not apply both. They record lease calls, deadline slack
  and stopped-authority causes without changing lease lengths or authority rules.
  Scheduling effects remain diagnostic. See
  [E24 diagnostic source](../docs/experiments/E24-lease-availability.md#diagnostic-source-and-ownership).

- `meridian-09-heap-diagnostics.patch`, `meridian-09-heap-diagnostics-r2.patch`,
  `meridian-09-heap-diagnostics-r3.patch`, `meridian-09-heap-diagnostics-r4.patch`
  and `meridian-09-heap-diagnostics-r5.patch`: E24 heap instrumentation on
  Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, after the reactor-1 stack
  ending at `meridian-06-io-pool.patch`. Each revision replaces the preceding
  overlay against that same base. Do not apply them cumulatively or add patch 08.
  Revisions 1 through 4 use DHAT; revision 5 uses allocation counters and sampling.
  Instrumentation can change performance, so these builds are not capacity or
  latency qualification. See
  [E24's proxy-memory handoff](../docs/experiments/E24-lease-availability.md#proxy-memory-handoff-to-e25).

- `horizon-13-lease-raw-clock.patch` and `horizon-13-lease-raw-clock-r1.patch`:
  E24's first clock fix on Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`,
  applied directly after the complete FINAL-09 stack. On macOS the keeper's
  suspend check compares MONOTONIC_RAW with UPTIME_RAW against one baseline taken
  before the claim, with a fixed 1 µs allowance and at most three resamples.
  It adds the macOS-only MIT `nix` 0.31.3 dependency (default features off,
  `time` only). Frozen as `lease-fixed-r1`. Superseded by revision 2 below; do
  not apply both. See
  [E24's clock fix](../docs/experiments/E24-lease-availability.md).

- `horizon-13-lease-renewal-r2.patch`: the accepted lease patch, self-contained
  on FINAL-09. It keeps the raw clock pair and adds a send-anchored renewal
  cadence: the next renewal is due 250 ms after the previous send, a failed call
  retries at once when that time has passed, and each call ends 50 ms before the
  keeper deadline. Deadline anchoring, fences and counters are unchanged. It
  also corrects the generic margin guard to include the clock allowance. Frozen
  with the bounded store as `lease-fixed-r2`, which passes the 30-minute soak
  (proxy memory excepted) and the full M3 campaign.

- `meridian-11-writev64-r2.patch` and its byte-equal frozen-build copy
  `meridian-11-write-batch-e25-scheduler2-writev64-r2.patch`: E25 on reactor-1
  plus the two patch-07 scheduler files. Replace the descriptor Vec with a
  fixed stack array, using 64 descriptors on macOS and the original 16 on
  other targets. FIFO order, partial offsets, the 256 KiB turn and queue caps
  stay unchanged. Scoped release gates pass; no latency or RSS pass is claimed.
  Revision 1 is retained as a failed build, not a selectable patch.

- `meridian-12-zone-pressure-r2.patch`: E25's build-gated, unmeasured zone-relief candidate.
  It calls the macOS allocator once per 60 seconds on the existing accept-waker
  thread, requesting 16 MiB without catch-up. A new MIT `meridian-zone` 0.1.0
  crate has no registry dependencies, build script, threads or runtime. It
  exports one safe pressure-relief API and returns zero off macOS. This is an
  approved local OS-boundary exception: crate-level `deny(unsafe_code)` and
  exactly one documented `allow(unsafe_code)` block. The proxy and all existing
  crates retain `forbid`. The exception is not selected if the candidate fails
  entry or memory qualification. Revision 1 is an incompatible direct-FFI draft.
  See [E25](../docs/experiments/E25-proxy.md) for provenance, gates and results.

- `goliath-store-e24-memory-r1.patch`: a record of E24's store change in this
  repository's own crates, already applied in `crates/goliath-store`. It bounds
  retry receipts to 9,216 FIFO entries, reuses the snapshot buffer and writes the
  snapshot frame without a second full copy, in the same on-disk format. A
  snapshot holding more receipts than the cap is refused at startup and needs an
  explicit migration.

- `meridian-10-mimalloc.patch`: E25 after frozen reactor-1's complete patch-06
  stack on `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`. Selects the global
  Rust allocator only, with pinned MIT `mimalloc` 0.1.52 and
  `libmimalloc-sys` 0.1.49, bundling native mimalloc 3.3.2. Optional features
  and runtime tuning stay off. It changes no routing, IO policy, thread count
  or authority rule. Both proxy builds pass scoped release gates, and clean
  application reproduces every recorded source input. Memory and latency
  qualification remain separate. See [E25](../docs/experiments/E25-proxy.md).

- `horizon-15-latency-stamps-e25-stamps-lease-r2-b2.patch`: E25's measurement-only
  timestamps on Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`, after
  FINAL-09 and `horizon-13-lease-renewal-r2.patch`. The bot stamps a movement
  packet immediately before its write; the tile stamps it in the connection
  reader right after decode, before the main-thread queue. Both use
  CLOCK_MONOTONIC_RAW. Direct-reference bots can sign their own backend login
  with the fixture's throwaway secret, which is never recorded. All gates pass,
  including the decode-before-dispatch test. Earlier `horizon-15-latency-stamps-*`
  revisions are retained as diagnostic or failed builds; b1 differs only in its
  test constructor. Not part of any production stack.

- `horizon-14-tick-timers.patch` through `horizon-18-tick-wait-detail.patch`, and
  `horizon-15-tick-timer-conditions.patch`: E26's measurement-only main-thread
  timers on FINAL-09, applied in order 14, 15, 16, 17, 18. They time every
  main-thread hunk of our patches and split pacing, worker wait and busy work.
  They are visibly intrusive, so they bound nothing by themselves. See
  [E26](../docs/experiments/E26-tick-profile.md).

- `horizon-19-lite-cpu.patch`: E26b's two-timer diagnostic on FINAL-09, or on
  FINAL-09 with patch 10. It reads the main thread's CPU clock once per cycle
  and once per speculative warm call. It stays inside run-to-run noise and
  measures warming at 5.24% of main-thread CPU without patch 10 and 0.38% with
  it. Measurement only.

- `horizon-10-warm-tick.patch` is now qualified as a warming-cost fix and is part
  of the final tile stack: FINAL-09, then `horizon-13-lease-renewal-r2.patch`,
  then patch 10. Frozen as `e26-final-stack-r1` with the bounded store and the
  rebuilt reactor-1 proxy.
