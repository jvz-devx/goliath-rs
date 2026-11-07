# E6: a fast coherent cut

## Verdict

The measured warm-cut pass lines passed. At 50 bots, capture stall P95/P99 was
2.231/2.413 ms, against 5/10 ms. Freeze-request-to-durable-reply P95 was
28.226 ms, against 40 ms. There were 100 samples at each of 1, 25 and 50 bots,
on normal populated terrain with changing chest slots and moving normal-AI pigs.
The largest complete request at 50 bots was 33.719 ms.

The final fast binary passed 200/200 SIGKILL/restart trials with zero mixed
images. The four markers and level tick agreed after every restart. Stale
fences could not append. A container-only change survived, with identical
block-state hashes before and after. The build with container tracking disabled
lost that change as required.

This is not a production D6 adapter. Accepted cuts are coherent within the
supported persisted representation and the stable resident footprint described
below. Changes to the player set, chunk unloads and accepted generation work
outside the resident snapshots fail closed. They need a restart and a new full
base. The cold bootstrap still blocks the tick thread for about 90 to 106 ms.
No shared player-store CAS or departed-generation recovery filter is wired here.

D6's update calls this log-structured policy P5. These results support its warm
cut budget. They do not establish a complete crossing input gap, bit-exact
simulation recovery or unrestricted streaming.

## Measured latency

Apple M5 Pro, 18 CPU threads, 48 GiB RAM, macOS and APFS. Each cell has its own
fresh game, Horizon home, launcher home and XDG directories under
`$REPO/.local/runs/`. Prepared 26.3 resources were read-only:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

The primary matrix uses release-grade optimization under the single `fast`
profile directory: opt-level 3, ThinLTO, one codegen unit and
`CARGO_INCREMENTAL=0`. All builds use `--no-default-features`.
The ordinary fast build has no LTO and 16 codegen units. Both binaries use the
same source.

There are 300 steady cuts plus three cold first cuts. Cuts run at about one per
second, with jitter to avoid locking requests to the 50 ms tick. Actual cell
rates were 0.989, 1.006 and 1.010 Hz. Each cell submitted over 8,400 real chest
slot writes. The fixture has 128 filled chests and 64 normal-AI pigs. Additional
generated containers, ambient entities and drops remain present. A pig's live
position changed during every cell, and the final chest query confirmed changed
diamond counts. Bots are real stationary creative clients, not walking players.

No compiler or linker process was present at either measurement boundary.
Every sample retains the load average. All three quiet cells stayed below the
machine's 18 CPU threads. The earlier three-sample, high-load development cell
is kept separately and is not pooled into this matrix.

Times are milliseconds. Percentiles use nearest rank. P99 over 100 samples is
the second-largest observation, not a long-soak tail estimate.

| Bots | Samples | Capture stall P50/P95/P99 | Freeze request to durable reply P50/P95/P99 | Request max | Load 1m |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100 | 0.558/1.814/1.902 | 15.994/28.351/30.016 | 32.065 | 2.47-4.17 |
| 25 | 100 | 0.668/1.864/2.403 | 14.597/25.908/31.979 | 35.975 | 2.40-5.91 |
| 50 | 100 | 1.188/2.231/2.413 | 20.637/28.226/32.608 | 33.719 | 5.20-8.92 |

`rtt_ms` starts before sending the single `tile-cut <token> <uuid>` request and
ends when its durable reply arrives. It includes control delivery, pacing,
freeze/export, capture, worker queueing, encoding, full flush and the reply.
It conservatively starts before the actual freeze. It is not a player-store
CAS or an end-to-end proxy crossing measurement.

`tick_ms` runs from the control handler's entry through snapshot preparation,
immediately before queue submission. The small enqueue cost is not isolated.
Native loop MSPT includes enqueue and all control-prelude work, including
control work done during pacing. The 50-bot loop maximum was 8.437 ms.

`capture_ms` excludes the initial freeze/menu handling and the cold bootstrap.
`off_ms` is the worker's batch work. `install_io_ms` includes append and the
full barrier, not just the flush syscall. The dispatch/reply residual below is
computed per sample as RTT minus server elapsed time. Component percentiles
are not added to make an end-to-end percentile.

| Bots | Snapshot body P95 | Worker encode P95 | Append plus full flush P95 | Worker total P95 | Dispatch/reply residual P95 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1.547 | 17.446 | 8.342 | 25.074 | 1.666 |
| 25 | 1.593 | 14.843 | 8.806 | 23.382 | 1.628 |
| 50 | 1.950 | 16.216 | 8.722 | 24.861 | 1.762 |

Raw distributions are in `data/E6/bench-release-grade-normal-{1,25,50}.json`.
Each contains all 100 per-cut observations, statuses, actual population,
load averages, churn evidence and compiler counts.

## Write volume and MSPT

Log bytes are the complete new checksummed record, including its header.
Kernel disk bytes use the same Darwin per-engine counter as E3 and include
codec staging and ordinary engine IO. The whole-cell average also counts work
between replies. Driver and bot log IO is outside that per-engine counter.

MSPT baseline is a ten-second trace of the same enabled server, with the same
churn and no cuts. Cut MSPT uses every loop trace in the roughly 100-second cell.
The observation windows are different and the world keeps evolving, so their
difference is not a paired estimate of E6 overhead. Pacing sleeps and Horizon's
existing terrain-pump wait are not MSPT. E6's idle control work is included.

Bytes below are KiB. Log and per-request kernel columns are P50/P95.
MSPT columns are P50/P99.

| Bots | Log KiB | Kernel KiB | Whole-cell mean kernel KiB/cut | Baseline MSPT | Cut MSPT | Loop max | Cut-loop ticks |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 113.5/140.9 | 144.0/176.0 | 152.1 | 0.666/1.132 | 1.032/3.748 | 8.635 | 2022 |
| 25 | 150.6/184.9 | 176.0/224.0 | 186.9 | 1.508/4.822 | 1.404/5.497 | 11.276 | 1987 |
| 50 | 187.6/242.7 | 224.0/272.0 | 227.0 | 1.866/2.783 | 2.831/7.287 | 8.437 | 1981 |

None of these 100-cut cells reached the production 64 MiB compaction threshold.
Their final logs were 16.35, 20.03 and 23.99 MiB and had no BASE yet. These are
short-run cut write volumes, not measured long-run compaction amplification.
Compaction recovery was tested separately, forced after every cut in the crash
campaign. Its latency under sustained concurrent traffic remains unverified.

### Cold bootstrap and actual population

These are single first-call observations, not percentiles. Loaded chunks are
baseline/cell-end. Containers and entities are counted in the initial persisted
image. Every populated fixture had all 128 chests and 64 tagged pigs.

| Bots | Resident chunks | Block entities | Saved entities | First request ms | First tick stall ms | First log MiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 153/153 | 339 | 84 | 205.099 | 90.078 | 4.60 |
| 25 | 155/155 | 358 | 86 | 217.348 | 106.111 | 4.90 |
| 50 | 155/155 | 358 | 86 | 218.215 | 96.929 | 4.94 |

A tile must finish this bootstrap before accepting a budgeted crossing. The first
record includes the ordinary saved tree plus current resident records, so it
writes some data twice. That cost was not hidden inside steady-state percentiles.

### Ordinary fast comparison

The final ordinary fast binary ran a separate quiet 50-bot, 20-sample cell.
Capture stall P50/P95/P99 was 0.890/1.032/1.080 ms and complete request latency
was 18.424/26.843/31.639 ms. Native cut-loop MSPT P50/P99 was 2.022/3.269 ms.
Load 1m was 10.76 to 11.91 and compiler counts were zero.

Both optimization settings passed. The 20- and 100-sample runs have independent
world timing and evolving populations, so they do not establish a compiler
speedup or slowdown. The release-grade matrix is the primary result.
See `data/E6/bench-fast-normal-50.json`.

## What changed

The patch still applies after `e2-horizon-input-trace.patch` on Horizon
`fc2868b2b26d56ec76e17414909fe58a3b87bb92`. It remains off without
`HORIZON_E3_CUT_DIR` and `HORIZON_E3_FENCE`. The legacy variable names are
retained. E3 HEAD archives are refused; E6 needs a fresh cut directory.

Capture stays in the dedicated-server adapter. Encoding and log/recovery are
separate from the ordinary region writers.

- Chunk mutations are detected through weak references to Horizon's COW chunks.
  A changed or replaced allocation is dirty. This does not consume
  `world.modified`, which ordinary autosaves can drain.
- The private block-entity map has opt-in per-chunk revisions on every mutable
  access path, including take/put, bulk mutation, removal, loading and attachments.
  An unproven mutable borrow conservatively counts as a change.
- Scheduled block/fluid ticks, LastUpdate and InhabitedTime use small chunk
  overlays. Empty tick lists are written too. They no longer force terrain
  serialization just because time advanced.
- Block-name changes conservatively dirty the resident chunks. Active moving
  pistons and gateway ages also force the relevant conservative capture paths.
- Live entities and every placed, unreleased player are captured at the same
  boundary. Guest stats and advancements are included. Menus and crafting grids
  close first so cursor items return to inventory or become captured drops.
- The tick thread still builds entity/player/metadata tags and the departing
  player's existing export record. It does not encode chunk NBT bytes or read
  the save tree on the warm path. Refactoring all Horizon entity serializers
  into clone-only builders was not needed for the measured stall budget.
- One persistent worker encodes a cut, appends one SHA-256-checked record and
  performs one `sync_all` plus macOS `F_FULLFSYNC` for the batch before replying.
  Up to eight pending cuts can share that barrier. Four simultaneous real
  requests were observed in one four-record group.
- `tile-cut <token> <departing-uuid>` calls the existing player-export freeze
  and captures without advancing a simulation tick. The reply carries the
  unchanged export record beside the durable cut receipt.
- The opt-in control prelude also runs while the server paces between ticks,
  checked about once a millisecond. This removes a nearly 50 ms alignment floor.
  Its work is carried into native MSPT. Ordinary servers retain the old wait.
- After replies, the worker compacts logs above 64 MiB into a BASE record.
  BASE is synced, atomically renamed and made durable before LOG is rotated.
  The codec's temporary directory is per process, so revoked workers cannot
  alter a new owner's codec input.

The initial bootstrap still uses E3's synchronous full-save barrier and tree
capture. Warm cuts reuse its unloaded saved state. A new player set, unload or
generation/proto-save revision is refused rather than accepting an untracked
image. Pending entity spawns are refused before freezing and can retry after a
tick. Lighting alone is not treated as a generation barrier. Native mods are
refused because their callback snapshot contract has not been established.

The generation observation adds a field/getter and counter notifications in
`horizon-client-render/src/streaming.rs`, including
`receive_decorated`, `receive_generated`, `receive_saved` and proto-save dispatch.
That file overlaps E7's area. No ticket-level, World::set, ChunkMap or ownership
policy was edited. A temporary-index application check passed for E2, then E6,
then E7. The combined code was not built or live-tested.

## Recovery and fencing evidence

The writer holds the same cross-process file lock as fence claims while it
checks the token and expected log identity/length, appends and flushes.
The inode check prevents a same-length ABA after compaction rotates LOG.
Sequences increase monotonically. A stale writer cannot append even an orphan
record. A failed worker refuses later cuts until restart.

Recovery takes the world's session lock before loading the game. It validates
the BASE and log prefix, drops a torn final frame, replays complete records,
rebuilds Anvil chunks with their metadata overlays and removes files born after
the selected cut. A corrupt installed BASE or interior log record fails closed.
An interrupted restore is repeated before accepting players on the next start.
The codec staging directories never select recovery.

The final fast binary completed 200 randomized SIGKILL/restart trials. The loaded
flat fixture has two marker players, a changing chest, one non-pickup dropped
stack and 16 caged normal-AI pigs. This retains E3's four-marker check while
adding moving mobs. The normal-terrain 1/25/50-bot matrix is the performance
campaign, not this crash fixture.

Every trial checked both players' inventory counts, chest count, dropped count
and the captured level tick from the recovered files. Both real bots then
rejoined and live exports/NBT predicates checked the four markers again.
There were 62 previous-cut and 138 next-cut recoveries, with zero mixed images.

The fixed seed is 20261107. Half the trials kill at the held gate. Half release
it and request a kill up to three milliseconds later. Actual scheduler delay
is not claimed to be exactly that requested delay. Compaction was forced after
each cut in this fault-only run.

| Gate | Trials | Meaning in E6 |
| --- | ---: | --- |
| capture-begin | 18 | Before snapshot preparation |
| captured | 15 | COW/tag snapshot prepared |
| pack-write | 17 | Before frame append, fence lock held |
| mid-append | 20 | Half of a record appended |
| pack-sync | 18 | Encoded, before acquiring the append/fence lock |
| head-write | 9 | Complete frame appended, before full flush |
| head-rename | 13 | Immediately before full flush, legacy gate name |
| head-renamed | 12 | Full flush finished, before receipt |
| installed | 12 | Sequence/image updated, before reply |
| base-write | 21 | Before writing BASE.new |
| base-sync | 17 | BASE.new written, before sync |
| base-rename | 12 | Before installing BASE |
| log-rotate | 16 | Durable BASE installed, before rotating LOG |
| Total | 200 | Zero mixed images |

Raw trials are in `data/E6/kills.json`. These are process kills, not power cuts,
media fault injection or a distributed ownership test.

The live fence test revoked work paused before its append lock. The old worker
returned `stale tile fence`; LOG was byte-identical. A pig and dropped stack
summoned outside resident chunks also refused the cut and kept the durable
sequence unchanged. See `data/E6/coherence.json`.

### Container-only positive and negative control

The test freezes simulation, warms the cut, then runs
`item replace block 8 -60 8 container.0 with minecraft:iron_ingot 31`.
It changes no block state. The block-state digest before and after recovery was
identical in both builds:

```text
d34022a964e415a9e336efb09e1cf8f818d6174efc82fe11d016773630406c76
```

| Build | Expected chest count | Recovered | Result |
| --- | ---: | ---: | --- |
| Tracking enabled, release-grade | 31 | 31 | Survives |
| HORIZON_E6_SKIP_CONTAINERS=1 at compile time | 31 | 9 | Lost as required |

The control leaves the other three markers intact. The old chest count is 9,
so an unchanged or empty fixture cannot pass the negative control.
See `data/E6/container.json` and `data/E6/container-control.json`.
Never use the control executable outside this fault test.

Six focused log tests passed on the final source. They cover every byte-truncated
tail, coherent restoration, interior/base corruption before restore, stale
fences, grouping, interrupted compaction, log-identity rotation and metadata
overlays replaced by newer terrain. The direct block-entity mutable-access test
also passed. The ordinary fast and release-grade binaries both passed the live
smoke checks.

## Commands and verification

From `$REPO/.local/src/horizon`:

```sh
CARGO_INCREMENTAL=0 cargo build --locked --profile fast --no-default-features -p horizon-client --bin horizon-client
HORIZON_E6_SKIP_CONTAINERS=1 CARGO_INCREMENTAL=0 cargo build --locked --profile fast --no-default-features -p horizon-client --bin horizon-client
CARGO_INCREMENTAL=0 cargo --config 'profile.fast.lto="thin"' --config 'profile.fast.codegen-units=1' build --locked --profile fast --no-default-features -p horizon-client --bin horizon-client
CARGO_INCREMENTAL=0 cargo clippy --locked --profile fast --no-default-features -p horizon-client --all-targets -- -D warnings
E3_TEST_ROOT="$REPO/.local/runs" CARGO_INCREMENTAL=0 cargo test --locked --profile fast --no-default-features -p horizon-client --lib game::dedicated::tile_cut::store::tests
CARGO_INCREMENTAL=0 cargo test --locked --profile fast --no-default-features -p horizon-client-render --lib block_entities::cut_tests
CARGO_INCREMENTAL=0 cargo fmt --all --check
```

All builds use the same target directory and profile name, serially. Copy each
finished executable before building the next configuration. Retained locations:

```text
.local/bin/horizon-e6/horizon-client
.local/bin/horizon-e6/control/horizon-client
.local/bin/horizon-e6/release-grade/horizon-client
```

From `$REPO`, using those retained binaries:

```sh
python3 -B scripts/e3-cut --e6 --section smoke
python3 -B scripts/e3-cut --e6 --section kills --kills 200
python3 -B scripts/e3-cut --e6 --section control --binary control
python3 -B scripts/e3-cut --e6 --section bench --samples 20 --only 50 --label fast
python3 -B scripts/e3-cut --e6 --section smoke --binary release-grade --label release-grade
python3 -B scripts/e3-cut --e6 --section bench --samples 100 --binary release-grade --label release-grade
```

Final strict Clippy passed in 39.91 seconds. The final six-test log run passed in
0.44 seconds after compilation. The mutable-access test passed. Scoped rustfmt
and `git diff --check` passed. Whole-workspace formatting failed only on E2's
existing input-trace hunk in `lan/hub.rs:4472`; it was left unchanged.
No full Horizon gameplay suite, vanilla parity suite or root-workspace suite
was run for E6.

The release-grade build took 304.25 seconds wall time. It still used
`target/fast`, not a second target or profile tree. Binary hashes, build flags
and the patch hash are in `data/E6/build.json`.

The exact measured drivers are retained as audit-only text snapshots.
Their hashes match the environment JSON for the 200-kill run, the fast cell and
the release-grade matrix. Do not execute those text files in place.
The delivered runner later added the explicit block-state digest assertion.
It passed both controls again. Benchmark and kill-loop behavior did not change.

## Failures and unverified state

- An early test-directory name collided between concurrent unit tests.
  A process-local atomic counter fixed it. The final six-test run passed.
- The first broad `terrain_loading` guard never settled on this headless server,
  even with no pending entity chunks. One run rejected 564 retries over
  60.103 seconds. The generation/proto-save revision replaced that guard.
- Strict Clippy initially caught a collapsible world-clock branch. It was fixed
  without a lint suppression.
- The first three-sample, high-load 50-bot development run reached request P95
  110.104 ms and worker P95 90.120 ms while load was about 37. It also serialized
  terrain for inhabited-time changes. Metadata overlays removed that work.
  This superseded cell is not an acceptance measurement.
- A shared codec staging directory could let a revoked process alter a new
  owner's codec input. Staging is now per process. The earlier 137 clean kill
  trials were stopped and retained separately; the final 200 were rerun after
  that fix.
- Cold first cuts miss the warm stall and latency limits. New player sets,
  unloads and newly accepted generation state are not supported incrementally.
  They refuse cuts rather than lose state. This adapter is not ready for normal
  login/logout and repeated production crossings without lifecycle tracking.
- Compaction runs after replies on the same worker. A later queued cut can wait
  behind it. Its behavior under sustained concurrent cuts, long-run write
  amplification and a larger tile were not benchmarked. Records are limited to
  512 MiB. This is not a production garbage-collection or long-soak result.
- Conservative block-name and moving-piston paths may re-encode many chunks.
  Active hoppers, redstone, piston chains, native mods, mod containers, open-menu
  overflow, dead/riding/sleeping players and continuous network menu interaction
  were not fault-tested. Inventory churn here is through real slot commands.
- Horizon's existing persistence omits or approximates runtime state. AI paths,
  parts of brains, per-entity RNG, numeric entity IDs, join ordering, pending
  neighbour/block-event queues and other transient state are not bit-exact.
  Existing unsupported entity persistence, including area-effect clouds, was
  not added. Scheduled ticks use Horizon's existing save/load semantics.
- Cross-dimension recovery, owner changes across machines, partitions,
  distributed fence issuance, ENOSPC, permission failures and physical power-loss
  durability were not tested. No generation-aware shared player-store recovery
  filter or CAS was integrated. Freezing only the departing player's inputs
  does not prove every external mutation of a frozen player safe.

E6 does not show that the warm cut needs the P4 fallback. If the unsupported
lifecycle or busy-world cases cannot fit the same cut budget, P4 would need a
durable tick/input batch before action acknowledgement, arrival anchors,
fencing on every append/CAS and compaction restricted to durable history.
That protocol was not built here. The cut timings alone cannot establish a
150 ms end-to-end crossing P95.

Only the allowed project notes and Horizon MIT source at the pinned revision
were used. No Folia, Paper, other GPL source or excluded research was read.
No launcher account, update, removal, data, share-log or settings command was
used. No commit, push or GitHub action was made.

## Disk and cleanup

The largest observed shared `.local` snapshot was 6,351 MiB. The largest
observed E6 Horizon target was 3,290 MiB. These are sampled maxima, not a
continuous filesystem high-water mark. Shared totals include other agents.
The largest primary run was 33,764 KiB, and the final crash run was 20,048 KiB.

Every E6 world/log directory was removed after its run. Only small raw results,
audit drivers, the patch and retained binaries remain. An earlier cleanup under
the original instructions freed 4,514 MiB: 3,290 MiB from E6's target and
1,224 MiB from the idle Horizon-tiles target. Those outputs are rebuildable.
The project was then 1,889 MiB and shared `.local` was 1,838 MiB. No active build
or user data was removed.

The reviewer then reserved `--all` for the orchestrator. The final task cleanup
used plain `scripts/clean-builds`, freed zero MiB and preserved the other agent's
new 397 MiB target under the cap. The project was 2,286 MiB. E6's target was
absent and there were no E6 run directories left. Both cleanup results are
recorded in `data/E6/build.json`:

```sh
GOLIATH_ROOT="$REPO" scripts/clean-builds
```

## Independent verification

The reviewer reran the release-grade 50-bot cell, the kill campaign and the control on 2026-11-07.

| Check | First run | Reproduction |
| --- | ---: | ---: |
| Capture stall P95, ms | 2.23 | 2.49 |
| Freeze to durable reply P50 / P95 / P99, ms | 20.6 / 28.2 / 32.6 | 21.6 / 30.9 / 33.5 |
| Kill trials passed, mixed images | 200 of 200, 0 | 200 of 200, 0 |
| Control build keeps the chest change | No | No |

The budget result stands. Its limit also stands: player set changes, chunk unloads and generation fail closed here, so this isn't usable with walking players until E10 removes that.
