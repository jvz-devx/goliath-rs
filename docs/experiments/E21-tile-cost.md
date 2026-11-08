# E21. Tile cost

Status: patch 10 is frozen, release gates and native recovery pass, and both
profiling rounds are complete. Combined M2 verification belongs to E20 and
is pending. No capacity or M2 acceptance is claimed.
Nothing was committed, pushed or sent upstream. No GPL source or excluded
research was read.

## Baseline at 650

The first cell uses FINAL-09 and E18's frozen reactor-1. The unchanged D9
fixture has 585 interior players and 65 crossing players, four 2,048-block
tiles, view distance 3, random tick speed 3 and 64 AI-enabled invulnerable
pigs per tile. Both server and bot limiters are disabled, so demand is not
hidden by SIGSTOP. Warm-up and measurement are 30 seconds each. Native
thread counters and a ten-second `sample` of tile a accompany the process
counters. This is a cost diagnostic, not an equal-budget frontier cell.

| Tile | Placed/held real sessions | Readers | Chunks | Process cores | Main cores | Cut-writer cores | Game-time TPS | Raw tick P99 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 164/2 | 65 | 368 | 0.56201 | 0.31427 | 0.05389 | 19.9822 | 26.414 |
| b | 154/1 | 65 | 367 | 0.56997 | 0.32025 | 0.04999 | 20.0151 | 36.157 |
| c | 165/3 | 65 | 367 | 0.63305 | 0.35187 | 0.05487 | 20.0119 | 39.058 |
| d | 167/5 | 65 | 367 | 0.56201 | 0.31941 | 0.04679 | 20.0088 | 37.686 |

Counts are sequential status-query observations, not an online population
sum. All 650 native bots stayed online. There were 298 activations during
the measurement. Start load was 3.8950 and the highest steady outside
process averaged 0.0644 core. Both steady and whole-cell noise gates passed.

The native main thread is the largest thread. Each tile's cut writer costs
0.047 to 0.055 core. The remaining cost is mainly network threads. Worker
threads for terrain generation are not the dominant steady cost.

Average per-tile CPU split, using matched native thread IDs:

| Work owner | Cores |
| --- | ---: |
| Main, including simulation, control and capture | 0.32645 |
| `horizon-net-*` | 0.19366 |
| `e6-cut-writer` | 0.05138 |
| `goliath-lease` renewal | 0.00011 |
| Generation workers | Below the displayed counter precision |
| Other threads and unmatched endpoints | 0.01015 |
| Total process counters | 0.58176 |

The two original diagnostic files retain inherited `server_cap_*` role lists
from E19. They are inventory metadata, not evidence of a running limiter.
Their before/after/whole-cell budget records explicitly have `disabled: true`
and zero stops. The exact executed runner is retained as
`data/E21/baseline650-runner.py`; its SHA-256 matches both raw runner hashes.
Later E21 files leave the cap role lists empty. No measured row was rewritten.

## The terrain-off switch is a confounded ablation

`E12_TERRAIN_OFF=1` removes all terrain readers, but also prevents successful
crossings in this fixture. Every crossing actor stays on a. There were zero
activations during the measurement, compared with 298 with terrain enabled.
This is retained as a diagnostic failure, not called reader-only savings.

| Tile | Placed/held real sessions | Readers | Process cores | Main cores | Cut-writer cores |
| --- | --- | ---: | ---: | ---: | ---: |
| a | 212/0 | 0 | 0.55677 | 0.37400 | 0.01820 |
| b | 146/10 | 0 | 0.40637 | 0.25269 | 0.02143 |
| c | 146/14 | 0 | 0.39973 | 0.25285 | 0.02164 |
| d | 146/28 | 0 | 0.41169 | 0.26017 | 0.02226 |

The off cell starts at load 3.5415, passes both noise gates and holds about
20 TPS. Its maximum steady outside average is 0.0578 core. Those validity
checks do not repair the changed handover workload or population placement.

## Stack evidence and first target

The frozen plain-09 reference with 163 players and 64 pigs costs 0.38702 core,
with 0.25819 on its main thread, 19.9963 TPS and tick P99 15.312 ms. It uses
one 128-by-128 patrol square, so it is a per-tile cost reference, not the D9
four-square denominator. Start load is 3.5371 and the largest steady outside
average is 0.0534 core. All native clients remained online and the cell is
valid. The D9 tile average is 0.58176 core, leaving a 0.19473-core difference
that includes readers, handovers and the different spatial layout. The
native main-thread difference is 0.06826 core. These are observed differences,
not a causal split of the residual.

`sample` includes blocked threads. The following are exclusive stack
occupancy counts, not CPU percentages and not an allocation of core seconds.
Native thread CPU counters above are the CPU evidence.

| Tile-a main-stack category | Readers on | Terrain off |
| --- | ---: | ---: |
| All observations | 7,760 | 7,247 |
| Waiting or blocked | 4,278 | 3,246 |
| Control, including inlined speculative warming | 633 | 973 |
| Player tick | 903 | 899 |
| Other world tick | 1,743 | 1,947 |
| Entity tracking | 113 | 85 |
| Chunk sending | 10 | 11 |
| Lease authority | 21 | 23 |

The control path is `Game::poll_server_control` called from
`Game::wait_for_next_server_tick_with`. The latter polls every millisecond
while pacing. Every poll calls `Game::tile_warm_generation`, which scans
resident chunks, hashes per-chunk string keys and sends speculative work
even when no simulation tick has changed. The scan is inlined into the
control symbol in the optimized binary. The worker uses `Encoded::chunk`
and `Encoded::proto`; the real cut additionally uses `encode` and `gzip`.

Entity tracking is `Hub::track_entities` and `Entities::tick`. Reader
sessions do participate as viewers, but this profile does not put them at
the top. Readers are already held and unplaced, so full real-player ticks
are not assumed to be reader work. Lease/authority stacks are small. No
precise ownership or digest CPU fraction is claimed from an inlined stack
sample. Digest hashing was not armed: every raw tick batch has zero digest
records, and `Game::e11_digest_tick` only calls `e11_region_digest` after an
explicit region request. The enabled raw tick recorder remains in both stacks.
The first optimization addresses speculative warming only.

## Patch 10

`horizon-10-warm-tick.patch` adds a once-per-world-tick admission check to
speculative generation warming. The one-millisecond control service stays
responsive. A worker still in flight does not consume the next tick's
opportunity; a completed pass cannot be resubmitted repeatedly in the same
tick. Three focused tests cover those cases and a backwards/reset tick.

The actual cut path is byte-unchanged. Its live-pointer, block-entity-revision,
moving-piston and gateway checks still decide whether the prepared snapshot
is usable. A cut after a same-tick mutation captures fresh state synchronously.
No ownership, lease, chunk delivery, entity tracking or player-tick rules change.
The private-index proof checks clean patch application after FINAL-09 and
the existing measurement stack, plus exact working-source bytes.

The additional `warm-cut` fixture warms an initial gold block snapshot,
freezes simulation, then commits diamond and emerald replacements in the
same world tick. It kills the tile and checks that the last emerald block
returns from recovery. This specifically exercises stale prepared images;
it supplements the unchanged E10/E12 regressions. The corrected native
recovery run passed, as recorded below.

## First post-change measurement

`warmtick2-650-1` repeats the uncapped 650-player D9 fixture with the frozen
first candidate. The proxy and support executable bytes are unchanged.
Warm-up and measurement remain 30 seconds each, with the same ten-second
tile-a stack sample. All 650 bots remained online. Both runs completed 298
activations in their measured window and retained 65 readers and 64 pigs on
every tile. No raw tick samples were dropped.

| Tile | Placed/held at start | Readers | Chunks | Before cores | After cores | After main cores | After cut-writer cores | After TPS | After tick P99 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 162/2 | 65 | 368 | 0.56201 | 0.55125 | 0.31388 | 0.02666 | 20.0057 | 41.226 |
| b | 153/1 | 65 | 367 | 0.56997 | 0.51873 | 0.30152 | 0.02016 | 20.0002 | 27.037 |
| c | 165/4 | 65 | 367 | 0.63305 | 0.57980 | 0.32341 | 0.03113 | 19.9988 | 41.230 |
| d | 170/1 | 65 | 367 | 0.56201 | 0.51143 | 0.29401 | 0.01882 | 19.9980 | 34.756 |

| Average per tile | Before cores | After cores |
| --- | ---: | ---: |
| Process | 0.58176 | 0.54030 |
| Main | 0.32645 | 0.30820 |
| Cut writer | 0.05138 | 0.02419 |
| Network workers | 0.19366 | 0.19661 |

The observed tile reduction is 7.1%, or 0.16582 core across four tiles.
Cut-writer CPU fell 52.9%. This is one valid before/after pair, not a repeated
capacity result or a proof that every tick tail improved. Tile a's tick tail
is higher in the second cell.

The control/warming category fell from 633 of 7,760 main-stack observations
to 69 of 7,134. Entity tracking is still small at 104 observations, against
113 before. Waiting accounts for 3,914 observations in the new sample.
These remain occupancy counts, not CPU shares.

Proxy CPU did not fall in this pair. Its aligned process average rose from
1.10609 to 1.21103 core. Consequently the complete production-server total,
including four tiles, proxy, store and sidecars, only fell from 3.44606 to
3.38485 cores, about 1.8%. No tile-only saving is presented as a cluster-wide
capacity gain. Bots and test-only shims remain outside this server sum.

The new cell starts at load 3.8462 and its largest steady outside-process
average is 0.05974 core. Steady and whole-cell noise gates pass. Its quiet
request lasts 112.397 seconds; the baseline request lasts 112.550 seconds.

## Regression failures retained

`warmtick2-freshness` successfully committed an initial image and three cuts
at world tick 46, then killed tile a. The adapter called `stop_child`, which
is for a different process wrapper and expects a `reader` attribute that
the server wrapper does not have. That harness exception stopped the test
before restart/recovery, so it is not a passed recovery test. Fixture cleanup
completed, the scratch world was removed and the quiet request was released.

The adapter now calls the server's own `stop()` API. A focused Python
regression covers that wrapper contract. The original failed result,
pre-cleanup native evidence and quiet report remain in `data/E21/`.

`warmtick2-freshness-2` passed with the unchanged frozen engine. After the
initial gold image, cut sequences 3, 4 and 5 all carry world tick 47. The
last two cuts replace the block with diamond and emerald. After SIGKILL
and restart, an independent block predicate finds the emerald block. The
35.238-second quiet window starts at load 3.9858 and its largest steady
outside-process average is 0.06934 core.

## Combined M2 handoff

The reviewer assigned the full M2 suite to E20 on one combined candidate,
using scheduler-2 with these warmtick-2 tiles. E21 does not run a separate
reactor-1 M2 suite. If the combined suite fails, each change will be tested
alone. The frozen tile directory, executable hash, patch hash and exact
source proof have been handed over. Combined M2 results are pending; the
local recovery pass is not a substitute for them. A second Horizon patch
is held until the controlled reader-cost results have been reviewed.

## Controlled reader cost

The reader-only comparison uses the unchanged frozen FINAL-09/reactor-1
bundle. It is a supplementary diagnostic, not the D9 capacity layout.
The four interior cohorts are unchanged. The 65-player crossing cohort now
walks a square from 16 to 24 on both axes inside tile a, safely outside the
four-block approach prediction. Both on and off therefore perform zero
handovers. This avoids removing real cut/CAS/activation work in only one arm.

Both arms retain 650 real players, placed counts 212/146/146/146 before and
after, zero held real players, and 64 AI-enabled pigs per tile. All bots stay
online. The on arm holds 65 readers per tile; off holds zero. Both pass the
explicit placement/no-handover contract, noise checks, roughly 20 TPS and
raw tick P99 below 50 ms.

| Tile | Reader-on cores | Reader-off cores | Reader mechanism delta | Main delta | Network delta | Cut-writer delta | Chunks on/off |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| a | 0.67946 | 0.60841 | 0.07105 | 0.03676 | 0.03118 | 0.00213 | 383/368 |
| b | 0.41279 | 0.38856 | 0.02423 | 0.01216 | 0.00428 | 0.00626 | 366/327 |
| c | 0.41246 | 0.38192 | 0.03054 | 0.03513 | -0.00684 | 0.00267 | 354/327 |
| d | 0.42408 | 0.38989 | 0.03420 | 0.01729 | 0.00938 | 0.00622 | 366/327 |
| Four tiles total | 1.92880 | 1.76877 | 0.16003 | 0.10134 | 0.03799 | 0.01729 | |

The observed reader-triggered tile cost is 0.04001 core per tile on average.
It includes the extra 15 to 39 resident chunks, their speculative preparation,
observer-position control, entity viewing and chunk/block delivery. It is
not a measurement of entity tracking alone. The negative network delta on c
is retained; thread-group and process endpoints have independent timestamps.

The proxy adds another 0.27197 core with readers enabled, 0.85514 versus
0.58317. The complete production-server difference is 0.43200 core,
2.78659 versus 2.35460. Store and sidecar changes are negligible in this pair.
This is one controlled pair with the original engine. Its CPU difference
must not be subtracted directly from the D9 crossing cell or extrapolated
to 975 players, because placement and handover work are different.

Start loads are 3.3906 and 3.0195; largest steady outside-process averages
are 0.06575 and 0.06509 core. Requests last 120.691 and 110.605 seconds and
are released between arms. The chunk table reports exact start counts. The on arm's
largest raw tick P99 is 30.413 ms and off's is 26.382 ms.

No second Horizon change was started. The reader mechanism is measurable,
but its main-thread increment also contains control and extra chunk work;
the profile does not establish a large entity-only saving. The reviewer has
the numbers before deciding the next optimization. The host is released for
independent frontier reproduction, and E21 has acknowledged that quiet window.

## Reproduction and evidence

Run from the repository root. Each command creates its own E21 quiet request
and waits for a fresh E20 acknowledgement and load at most 4. Release occurs
after cleanup. Use new result names for repeat runs.

```sh
python3 -B scripts/e21-tools-test
python3 -B scripts/e21-tile-profile --bins .local/bin/meridian-e18/reactor-1 --counts 650 --warmup 30 --seconds 30 --sample --name baseline650-readers-on-1
python3 -B scripts/e21-tile-profile --bins .local/bin/meridian-e18/reactor-1 --counts 650 --warmup 30 --seconds 30 --sample --terrain-off --name baseline650-readers-off-1
python3 -B scripts/e21-plain-profile --players 163 --name baseline-plain163-1
```

Raw results and sanitized stack summaries live in `data/E21/`. Full sample
reports remain local in `.local/evidence/E21/`. Outside-process names are
`outside` in published evidence; PIDs and CPU counters remain intact.
Worlds and logs are temporary under `.local/runs/` and removed by the fixture.

The source stack is proved in a private Git index by `scripts/e21-stack`.
It applies the pinned FINAL-09 stack and the existing measurement patches
without creating a third Horizon checkout. The older dirty `horizon-tiles`
tree is preserved. E21 uses the full E12 tree at `.local/src/horizon`.

The remaining M2 outcomes will be added after those gates complete.

### Build transport interruption

The first native pipeline, `warmtick-1`, passed whole-workspace formatting and
all three focused release unit tests. The unit command took 350.319 seconds.
The tool transport timed out at 300 seconds, but the original Cargo process
continued. A native kqueue exit wait confirmed its completion. The runner then
hit `BrokenPipeError` writing to the closed tool output pipe; this is retained
in `warmtick-1-build.json`, not relabelled as a code failure or a complete build.

`warmtick-2 --resume-from warmtick-1` verifies identical source-proof and patch
hashes, reuses only successful completed gates, and runs the remaining Clippy
and release-binary commands. The unit command was not rerun and no two Cargo
commands used this workspace concurrently. Strict release Clippy passed in
32.711 seconds. The release-binary command passed in 248.298 seconds.

### Frozen first candidate

Directory: `.local/bin/horizon-e21-fc2868b2b/warmtick-2/`.

| Artifact | SHA-256 |
| --- | --- |
| `horizon-client` | `ea4537868c90771e0d030ccfff1dbe36dace215a2c70d652dd86b38b84434090` |
| Patch 10 | `2ea13f3e6941a5b56306bde91943e42e9b520814378f9f692a45427ec48d4ce4` |
| `meridian`, unchanged reactor-1 | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| `meridian-sidecar` | `c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b` |
| `horizon-bots` | `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d` |
| `goliath-store` | `6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af` |
| `e4-client` | `e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025` |

The source manifest, build record, leased marker, executable manifest and
frozen launcher accompany the binaries. This is a release build candidate,
not a completed M2 or capacity result.

## Evidence readback and cleanup

Final readback matches all six frozen executable hashes and all 60 compiled
source identities. Both contained-cohort arms retain their exact placed,
held, reader and pig counts before and after measurement. Every published
outside-process name was checked to be `outside`, including phase rows and
the PID/name maps. The six lightweight Python contract tests pass, and new
script/report files pass whitespace checks. The Rust release checks are the
scoped commands recorded in `warmtick-2-build.json`; root crates were unchanged.

| Raw evidence under `data/E21/` | SHA-256 |
| --- | --- |
| `baseline650-readers-on-1.json.gz` | `96255e972a63ae8d25472a078378d8fdf164a6dbdcb2edfe3bd3b1144f01891a` |
| `baseline650-readers-off-1.json.gz` | `9c9231263a8b283fe74589005820e5d6a75bda01d35f85b6cd67787c1a99933a` |
| `baseline-plain163-1.json` | `190a92db39128db1ff12e9319b429319c1b81afbadd2f448d6094ff236468597` |
| `warmtick2-650-1.json.gz` | `c867788db6010dd73520f3e9d2508562e5153a0d5468d84ba6aea6591e9868d8` |
| `warmtick2-freshness-2.json` | `d83244ae8325044eeacca136a8bacbb0c053eb818ba030db04dff9af95ea538c` |
| `contained650-readers-on-1.json.gz` | `033addc1ddd09cffe0ff41771ca3b81cb203785bccd872f3dc7b8908358166d3` |
| `contained650-readers-off-1.json.gz` | `cb60ec98a9aa79b5f7859f87dbb70e5193111db28fb4b2dccb25204d2ec18e43` |

All run worlds and logs were removed after retaining evidence. Plain
`scripts/clean-builds`, without `--all`, completed and freed 0 MB. It reported
one Horizon target at 1,870 MB, Meridian at 566 MB and the root at 38 MB,
all below their caps; the project total was 7,438 MB. No second Horizon
checkout or target was created. The older dirty Horizon-tiles tree and
E20's source changes were preserved.

E21 has released the host and left its quiet acknowledgement for the
reviewer's independent frontier reproduction. No build, load or second
Horizon patch is queued. Combined M2 verification remains pending with E20.
No normal-terrain freeze-latency or new equal-budget frontier claim is made.

## Frozen paths and exact commands

Frozen tile binary:
`.local/bin/horizon-e21-fc2868b2b/warmtick-2/horizon-client`

SHA-256:
`ea4537868c90771e0d030ccfff1dbe36dace215a2c70d652dd86b38b84434090`

Patch: `patches/horizon-10-warm-tick.patch`, SHA-256
`2ea13f3e6941a5b56306bde91943e42e9b520814378f9f692a45427ec48d4ce4`.
The matching source proof is `data/E21/stack-proof.json`, SHA-256
`de438131a8d7be828cb36eb6cf18ac1de13f84bda7f50a8269d33f1d67bcb15e`.

From the repository root, with fresh result names and a fresh E20
acknowledgement for each measurement window:

```sh
python3 -B scripts/e21-tools-test
python3 -B scripts/e21-tile-profile --bins .local/bin/horizon-e21-fc2868b2b/warmtick-2 --counts 650 --warmup 30 --seconds 30 --sample --name reproduce-warmtick2-650
python3 -B scripts/e21-tile-profile --bins .local/bin/meridian-e18/reactor-1 --counts 650 --warmup 30 --seconds 30 --contained-crossers --name reproduce-contained650-on
python3 -B scripts/e21-tile-profile --bins .local/bin/meridian-e18/reactor-1 --counts 650 --warmup 30 --seconds 30 --contained-crossers --terrain-off --name reproduce-contained650-off
python3 -B scripts/e21-window --name reproduce-warmtick2-freshness -- python3 -B scripts/e21-check warm-cut --bins .local/bin/horizon-e21-fc2868b2b/warmtick-2 --name reproduce-warmtick2-freshness
scripts/clean-builds
```

To rebuild identical source in the already-proved owned checkout, outside
any measurement window, use a new freeze name:

```sh
python3 -B scripts/e21-stack
python3 -B scripts/e21-build --name reproduce-warmtick
```

## Stored raw data

Raw result files larger than 1 MB are stored gzip-compressed as `*.json.gz`. Compression is lossless, and every SHA-256 recorded in this report refers to the decompressed bytes. To rerun a summary script on one of them, decompress it first with `gzip -dk <file>.json.gz`.
