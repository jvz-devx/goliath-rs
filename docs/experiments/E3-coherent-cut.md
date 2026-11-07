# E3: a coherent tile cut

## Verdict

The current cut/RPC pipeline does not fit the frozen-window reference on normal
terrain. Direct request-to-install P95 reached 160.2 ms at 50 bots. An approach
cut followed by a coherent frozen delta still took 169.7 ms P95. The lower-load
one-bot repeat also missed, at 148.7 ms direct and 156.1 ms for the frozen delta.
Flat-world results alone would have given the wrong answer.

The distinction matters: internal durable work, excluding the initial control
queue, had a maximum steady-cell P95 of 119.5 ms. The direct tick-thread stall P95
reached 89.0 ms. Those components fit the 128 ms work reference on this Mac,
but the measured complete RPC does not fit E1's 136 ms durable-checkpoint
reference once tick alignment and control delivery are included. The approach
pre-cut does not leave a cheap enough delta in this implementation.

D6 is not validated for this frozen-window path. P4 is its planned fallback if
this path is retained. These measurements do not prove P2 impossible: a same-tick
freeze/cut dispatch and a genuinely small in-memory semantic delta could change
the result. Neither was built or measured here. Do not adopt this prototype as a
production D6 implementation.

Independent world/player saves were not substituted to improve the numbers.
Accepted cuts cover supported persisted state at one boundary. Missing resident
live state fails the request instead of producing a partial image. This is not
proof that every Horizon runtime field needed by a production tile is covered.
The serializer omissions below remain, and the adapter is not a general snapshot
of an arbitrary incomplete streaming state.

## What was built

Horizon is pinned at `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. Apply
`patches/e2-horizon-input-trace.patch`, then
`patches/horizon-01-tile-cut.patch`. Nothing was committed or sent upstream.
The E3 patch passes `git apply --cached --check` against that clean tree after E2.
The check used a temporary index, not another Horizon checkout.

The change is off without `HORIZON_E3_CUT_DIR` and `HORIZON_E3_FENCE`.
The former names an experiment-owned checkpoint directory outside the working
world. The latter binds the process to an opaque token. The control endpoint adds:

```text
tile-fence <expected-token> <new-token>
tile-cut <process-token>
```

`-` means an unclaimed fence. Claims use CAS. Tokens must never be reused.
A request must match the process's token. Installation checks the current fence
and the captured HEAD sequence under the same cross-process file lock used by
claims. Revoked work may leave an unreachable pack, but cannot publish a cut.
Only one cut can be in flight per server. Installed sequences increase by one.

Capture runs between simulation ticks, in the dedicated control prelude. It:

1. Drains the existing generation save barrier, including parked dimensions.
   A 30-second timeout fails the request instead of acknowledging a partial save.
2. Closes resident menus and crafting grids. Their cursor stacks and grid contents
   must go back into inventory or into world drops before capture.
3. Includes every loaded chunk, not only `world.modified`. Block entities and
   containers have mutable state independent of block-state edits.
4. Uses Horizon's existing chunk snapshots, entity serializers and region writer.
   It waits for those writes and every resident player snapshot. No tick advances.
5. Reads the working save tree into immutable memory. Unloaded saved chunks,
   entity regions, POIs, level data and ordinary saved-data files are included.
   Resident statistics and advancements are serialized directly into that image.

The last step avoids the transfer-arrival helper's separate fsync for every
statistics and advancement file. Their durability belongs to the cut's shared
barrier. Serialization errors still fail the cut.

There is a hard precondition. The E3 save path scans all supported live entity
positions, containers, moving pistons and remembered unknown entity chunks.
Every one must have a resident chunk. Otherwise the request fails and keeps the
previous HEAD. It does not write a partial cut or merge live entities over an
unloaded chunk's saved residents. The caller must load the missing chunks before
retrying. Already-unloaded ordinary save data remains included in the file tree.
This prototype does not force missing chunks resident itself.

Off-thread installation hashes 64 KiB content pages. Pages present in the
previous manifest are reused. New pages go into a new immutable pack. The writer
syncs the pack, writes and syncs a complete HEAD manifest, then renames HEAD once.
It syncs the directory and flushes again before success. On this Mac, each file
barrier uses `sync_all` plus `F_FULLFSYNC`, not plain fsync alone. HEAD contains
the token, sequence, game tick and the complete file-to-page mapping.

Only HEAD selects a cut. Recovery does not scan pending manifests or orphan packs.
It verifies every referenced page before replacing working-save files. Files born
after the cut are removed. The normal world session lock is held during recovery,
and recovery finishes before the game loads the world or accepts players. A kill
during recovery can repeat the same restore on the next start. Missing or corrupt
referenced data fails startup instead of silently loading part of another cut.

This is incremental at the archive page level. It still serializes resident
chunks and scans the entire saved world on the tick thread. It does not have
semantic chunk dirtiness tracking or a constant-time Arc-only capture. Packs have
no garbage collection. These limits matter for larger tiles and long runs.

## Measurement setup

Apple M5 Pro, 18 CPU threads, 48 GiB RAM, macOS, APFS. All game files and logs
were in fresh owned directories under `$REPO/.local/runs/`, removed after each
run. Each server had fresh Horizon, launcher and XDG homes. Prepared resources
were opened from:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

No launcher account, update, removal, settings or data commands were used.
No game files were copied into Git.

The comparison is the retained, uninstrumented
`.local/bin/horizon-fc2868b2b/horizon-client-baseline`, not the E2 trace binary.
Both servers use seed 12345, view distance 3, simulation distance 3, creative
players, offline profiles behind Horizon's test Velocity relay, and the same
retained real `horizon-bots`. The bots are stationary. They are not mocked player
records or a survival workload. The driver records their online and received-chunk
counts as well as server player and loaded-chunk counts.

The empty case is a fresh flat world with bedrock, two dirt layers and grass.
The populated case uses normal generated terrain, with `generate-structures=false`
configured. Extra generated containers are still counted, not assumed absent.
After bot views load, a platform at Y=300 supports 128 filled chests and 64
normal-AI pigs. Each chest contains nine diamonds. Fixture pigs carry an audit
tag. Normal-world bots are teleported onto the platform and use `stand`; flat
bots use `idle`. Actual saved populations are checked independently, including any extra
generated state. Difficulty is peaceful. Other gamerules stayed at the pinned
defaults: respawn radius 10, mob spawning and mob drops on, weather advancement
on. Four legacy gamerule names in the measured driver were rejected and changed
no state. This was verified in saved `game_rules.dat`, not inferred from the
requested commands. The exact measured runner is retained as
`data/E3/driver-measured.txt`, SHA-256 `71aa0c38...`, for audit only. Do not run
that snapshot in place. See `data/E3/setup-validation.json`.

After measurement, only those rejected no-op lines were removed from the
delivered runner. Its SHA-256 is `9b5fc18a...`. Capture, benchmark and kill-test
logic did not change. The delivered runner passed the live smoke checks again.

Ambient entities and drops therefore remain in the measured workload. Their
counts are reported. There is no chest interaction or hopper churn in the latency
run. The recovery fixture uses one explicit identity-marked dropped stack.

There are 20 cuts per world, bot count and interval. Intervals are explicit waits
after the previous reply: zero, one or five seconds. The first full cut is kept
separately and excluded from those steady-state samples. Percentiles use nearest
rank. With 20 samples, P99 is the maximum, not a precise tail estimate.

`rtt_ms` starts before sending the control request and ends at its durable reply.
It includes tick alignment and control queueing. `tick_ms` is wall time spent
blocking the tick thread, including waits for ordinary region/player writers.
It is not CPU time. `off_ms` covers page hashing, pack IO and installation after
the immutable image exists. `server_ms` excludes the initial control queue.
`install_io_ms` includes writes and lock wait, not only fsync syscalls.

Archive bytes are new pack payload plus the complete HEAD manifest. They exclude
ordinary working-save writes. The separate disk byte column uses Darwin
`proc_pid_rusage` disk-write counters for the entire engine, including its writer
threads. This prevents archive deduplication from hiding ordinary-save IO.

MSPT comparisons use Horizon's existing last-100-tick status window, which includes
the control prelude. Its P99 selects the largest sample. Whole-run loop P50/P99,
maximum and sample counts are also retained. A cut every five seconds can occupy
less than one percent of ticks, so a whole-run P99 alone can hide the stall.
Raw files contain load averages at each sample. Other experiment agents were
running on this Mac. The final crash campaign and the final latency matrix were
run sequentially, with no Horizon compiler running during the final matrix.

## Latency, bytes and MSPT

All values below are measured on the final binary. The primary matrix has 360 steady-state cuts, plus six initial full cuts and 60 approach/frozen-delta pairs. Each cell has 20 samples. Times are milliseconds. Load is the observed one-minute range during that cell.

### Request, tick-thread and off-thread latency

| World | Bots | Wait s | Request P50/P95/P99 | Tick stall P50/P95/P99 | Off-thread P50/P95/P99 | Server P95 | Load 1m |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flat | 1 | 0 | 50.0/55.9/82.5 | 20.7/21.3/22.1 | 17.1/21.9/22.9 | 42.7 | 2.4-2.4 |
| flat | 1 | 1 | 50.0/100.9/102.3 | 22.1/30.2/30.5 | 17.5/25.7/32.5 | 52.6 | 2.4-3.8 |
| flat | 1 | 5 | 98.3/105.2/105.9 | 34.2/36.0/38.6 | 17.3/28.1/32.5 | 62.3 | 3.9-5.5 |
| flat | 25 | 0 | 53.1/97.0/107.0 | 27.8/30.2/32.1 | 20.0/24.1/28.1 | 51.8 | 5.4-5.8 |
| flat | 25 | 1 | 54.0/99.7/102.9 | 29.4/37.5/37.9 | 15.1/19.2/19.9 | 54.7 | 5.8-6.8 |
| flat | 25 | 5 | 96.2/102.2/104.8 | 38.1/43.5/43.8 | 16.6/21.2/21.6 | 61.4 | 6.5-7.8 |
| flat | 50 | 0 | 99.1/104.0/107.0 | 36.1/38.6/38.8 | 19.3/22.1/22.2 | 59.6 | 6.5-6.6 |
| flat | 50 | 1 | 96.6/105.4/113.4 | 40.4/45.9/46.7 | 19.3/24.9/26.6 | 70.5 | 6.6-9.1 |
| flat | 50 | 5 | 98.1/102.1/110.6 | 47.0/50.6/53.5 | 19.6/23.0/23.4 | 71.7 | 5.8-8.9 |
| normal | 1 | 0 | 99.6/105.9/106.1 | 42.6/44.6/48.0 | 25.7/29.7/30.1 | 73.0 | 4.1-4.4 |
| normal | 1 | 1 | 98.1/123.6/137.9 | 61.5/67.0/70.2 | 27.0/38.7/41.6 | 95.3 | 3.3-4.1 |
| normal | 1 | 5 | 106.8/144.6/156.7 | 59.4/70.8/71.2 | 33.5/42.4/43.4 | 106.1 | 3.0-21.2* |
| normal | 25 | 0 | 99.9/105.8/106.8 | 52.6/54.9/57.4 | 24.7/27.1/27.2 | 80.4 | 9.6-9.6 |
| normal | 25 | 1 | 109.4/143.4/153.4 | 64.0/76.9/77.9 | 26.6/34.0/35.8 | 103.5 | 7.4-9.6 |
| normal | 25 | 5 | 138.6/159.5/162.5 | 77.6/80.8/81.3 | 29.8/31.6/35.1 | 112.8 | 3.1-7.0 |
| normal | 50 | 0 | 100.8/106.9/110.1 | 59.2/61.2/62.0 | 26.2/29.3/30.5 | 88.8 | 3.6-3.6 |
| normal | 50 | 1 | 135.3/157.4/161.3 | 76.2/85.0/90.5 | 25.6/34.6/36.1 | 116.8 | 3.6-4.1 |
| normal | 50 | 5 | 138.4/160.2/167.8 | 83.2/89.0/93.2 | 28.2/36.1/39.8 | 119.5 | 2.2-3.5 |

The marked normal-one-bot cell exceeded the Mac's 18 CPU threads in load average. It is repeated below at lower load. The primary result is retained, not replaced or pooled.

### Write volume per cut

Archive bytes count the new delta pack plus HEAD. Kernel bytes include the working save as well. Both columns are KiB, P50/P95. The initial full cut is excluded.

| World | Bots | Wait s | Archive KiB P50/P95 | Engine disk KiB P50/P95 |
| --- | ---: | ---: | ---: | ---: |
| flat | 1 | 0 | 537.1/537.1 | 1096.0/1096.0 |
| flat | 1 | 1 | 537.1/573.5 | 1096.0/1172.0 |
| flat | 1 | 5 | 601.7/602.0 | 1224.0/1232.0 |
| flat | 25 | 0 | 658.8/659.0 | 1408.0/1408.0 |
| flat | 25 | 1 | 660.8/697.2 | 1408.0/1480.0 |
| flat | 25 | 5 | 697.2/697.5 | 1476.0/1484.0 |
| flat | 50 | 0 | 689.6/689.7 | 1540.0/1540.0 |
| flat | 50 | 1 | 693.2/745.6 | 1540.0/1644.0 |
| flat | 50 | 5 | 746.1/746.5 | 1644.0/1652.0 |
| normal | 1 | 0 | 2135.5/2147.5 | 4000.0/4028.0 |
| normal | 1 | 1 | 2179.6/2211.8 | 4100.0/4156.0 |
| normal | 1 | 5 | 2220.0/2232.1 | 4164.0/4196.0 |
| normal | 25 | 0 | 2137.4/2147.5 | 4096.0/4108.0 |
| normal | 25 | 1 | 2197.5/2221.0 | 4220.0/4264.0 |
| normal | 25 | 5 | 2237.5/2253.4 | 4308.0/4340.0 |
| normal | 50 | 0 | 2161.8/2172.6 | 4220.0/4264.0 |
| normal | 50 | 1 | 2194.1/2234.1 | 4276.0/4368.0 |
| normal | 50 | 5 | 2255.0/2267.1 | 4416.0/4444.0 |

The archive is page-incremental, but the data does not become a small semantic delta. Normal-world cuts still add about 2.1 to 2.2 MiB to the archive and cause about 4 MiB of engine disk writes. Resident chunks are reserialized every time. A pre-cut does not remove that work.

### Actual footprint and first full cut

Chunks are baseline/cut, with the cut range taken from the three cell-end statuses. Block entities and saved entities are counts in the initial saved images, baseline/cut. Every populated image passed the separate 128-chest and 64-tagged-pig checks. Worlds share a seed and configuration, but are not cloned, tick-identical images.

| World | Bots | Loaded chunks base/cut | Block entities base/cut | Saved entities base/cut | First request/server ms | First archive MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| flat | 1 | 121/121 | 0/0 | 0/0 | 78.5/38.7 | 0.51 |
| flat | 25 | 144/144 | 0/0 | 0/0 | 83.4/43.7 | 0.63 |
| flat | 50 | 144/144 | 0/0 | 0/0 | 92.6/50.7 | 0.66 |
| normal | 1 | 153/153 | 339/339 | 84/84 | 188.6/145.1 | 3.29 |
| normal | 25 | 155/155 | 358/358 | 86/86 | 185.2/143.9 | 3.56 |
| normal | 50 | 155/155 | 358/358 | 86/86 | 180.3/140.3 | 3.60 |

The cold normal cuts took about 180 to 189 ms request-to-install. A tile must make its initial cut before advertising a budgeted crossing. The six single first-call values are not percentile estimates.

### Tick MSPT while cuts run

Baseline is Horizon's last-100-tick status window. Cut values use every tick-loop trace sample in that cell, including control work. Their maximum is included because at five-second spacing the checkpoint stalls can fall below one percent of ticks. Native last-100-window cut statistics are also retained in the raw JSON. These are different observation windows, not a paired tick replay.

| World | Bots | Wait s | Baseline P50/P99 | Cut loop P50/P99 | Cut max | Cut ticks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| flat | 1 | 0 | 0.28/0.63 | 21.12/22.48 | 22.48 | 20 |
| flat | 1 | 1 | 0.28/0.63 | 0.31/30.63 | 30.92 | 426 |
| flat | 1 | 5 | 0.28/0.63 | 0.33/3.92 | 39.11 | 2040 |
| flat | 25 | 0 | 0.73/3.66 | 28.49/33.11 | 33.11 | 27 |
| flat | 25 | 1 | 0.73/3.66 | 0.78/36.66 | 39.05 | 431 |
| flat | 25 | 5 | 0.73/3.66 | 0.93/4.79 | 44.97 | 2041 |
| flat | 50 | 0 | 1.40/1.91 | 35.53/40.42 | 40.42 | 39 |
| flat | 50 | 1 | 1.40/1.91 | 1.48/46.63 | 48.50 | 441 |
| flat | 50 | 5 | 1.40/1.91 | 1.50/6.23 | 55.20 | 2041 |
| normal | 1 | 0 | 2.06/3.11 | 0.67/48.80 | 48.80 | 40 |
| normal | 1 | 1 | 2.06/3.11 | 2.09/64.75 | 71.30 | 441 |
| normal | 1 | 5 | 2.06/3.11 | 0.96/6.18 | 72.24 | 2046 |
| normal | 25 | 0 | 3.41/5.21 | 1.79/59.07 | 59.07 | 40 |
| normal | 25 | 1 | 3.41/5.21 | 2.17/72.09 | 80.12 | 447 |
| normal | 25 | 5 | 3.41/5.21 | 3.84/11.45 | 83.69 | 2057 |
| normal | 50 | 0 | 5.29/7.37 | 2.29/64.32 | 64.32 | 40 |
| normal | 50 | 1 | 5.29/7.37 | 4.03/81.33 | 93.13 | 456 |
| normal | 50 | 5 | 5.29/7.37 | 4.91/10.44 | 95.97 | 2059 |

### Approach pre-cut, then a frozen delta

Where a direct cell crossed the reference, the driver made an approach cut, waited 250 ms, froze E3Bot0 through `player-export`, then requested another complete coherent cut. It released the freeze only after the durable reply. There was no player-only shortcut. There are 20 pairs per populated case.

| Bots | Frozen delta request P50/P95/P99 | Frozen delta tick P50/P95/P99 | Frozen delta archive KiB P50 | Freeze-to-install upper P50/P95/P99 |
| --- | ---: | ---: | ---: | ---: |
| 1 | 135.8/142.1/143.5 | 57.8/63.9/64.5 | 2232.0 | 154.1/165.2/166.9 |
| 25 | 153.3/168.0/177.2 | 76.8/85.0/90.4 | 2248.5 | 170.4/215.8/222.6 |
| 50 | 157.5/169.7/179.6 | 81.9/92.2/93.9 | 2259.9 | 195.0/216.3/229.0 |

The upper-window measurement starts before sending `player-export`. It includes any queue before the actual freeze and stops at cut installation, before a player-store CAS. It is not an exact freeze timestamp or a complete crossing measurement. Delta request-to-install latency itself is measured directly.

Primary raw data: `data/E3/bench.json`. The lower-load repeat is separate below. No component percentiles were added to construct a request percentile.

### Lower-load repeat of normal, one bot

Same final binary and measured driver, 20 cuts per interval and 20 pre-cut pairs. Both initial saved images again had 339 block entities and 84 entities, including the 128 fixture chests and 64 fixture pigs. There were 153 resident chunks. This repeat is not pooled with the primary matrix.

| Wait s | Request P50/P95/P99 | Tick stall P50/P95/P99 | Off-thread P50/P95/P99 | Load 1m |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 98.8/112.9/115.1 | 42.8/46.5/46.8 | 27.0/36.4/37.9 | 2.52-2.52 |
| 1 | 104.0/142.6/148.9 | 58.2/65.8/67.9 | 26.4/40.2/41.2 | 2.05-2.48 |
| 5 | 120.4/148.7/149.3 | 66.4/73.3/73.6 | 29.4/36.2/39.5 | 1.49-1.89 |

The repeat's frozen delta request P50/P95/P99 was 143.2/156.1/163.4 ms. Its tick-stall P50/P95/P99 was 69.5/73.5/74.5 ms. The freeze-to-install upper window was 168.6/199.9/205.6 ms. The complete-request miss remained at lower load.

Raw repeat: `data/E3/rerun-populated-1.json`; environment and exact command: `data/E3/environment-rerun-populated-1.json`.

## Real fsync

Two hundred serial 64 KiB overwrite-and-sync samples per mode, on this repository's
filesystem. These are measured disk-barrier calls, not E1 assumptions. Full flush
includes fsync followed by Apple's `F_FULLFSYNC`.

| Barrier | P50 ms | P95 ms | P99 ms |
| --- | ---: | ---: | ---: |
| fsync | 0.0247 | 0.0318 | 0.0342 |
| fsync plus full drive-cache flush | 2.9968 | 3.8159 | 4.0354 |

Raw samples are in `data/E3/fsync.json`. The benchmark does not inject an 8 ms
storage delay. E1's 128 ms work limit and 136 ms complete-checkpoint reference
are model budgets. This adapter uses multiple barriers, so replacing its IO with
one assumed 8 ms call is not a valid performance claim. No end-to-end crossing
or player-store CAS is timed here.

## Recovery and fencing

The final guarded binary completed 200 randomized SIGKILL/restart trials with zero
mixed images or partial-cut loads in the loaded flat fixture.
A fixed seed, 20261107, chooses a crash gate. Half
the trials kill at that gate. Half release it and kill up to three milliseconds
later, covering work between gates too. Gates are enabled only for fault tests,
never for the latency matrix.

| Chosen gate | Trials |
| --- | ---: |
| Before capture | 27 |
| Immutable image ready | 28 |
| Before pack write | 23 |
| Before pack sync | 26 |
| Before HEAD write | 23 |
| Before HEAD rename | 18 |
| After HEAD rename | 31 |
| Installed, before reply | 24 |
| Total | 200 |

There were 134 previous-cut recoveries and 66 next-cut recoveries. Both are valid
on opposite sides of the atomic installation. Every trial checked the manifest
sequence, restored level time, one container, one non-pickup dropped stack and
two different players' inventory counts. Marker values changed before the next
cut, so mixing files from adjacent cuts fails the check. After each restart,
both real bots rejoined. Inventory exports and live NBT predicates checked the
restored inventories, chest and dropped stack again. Raw trial records are in
`data/E3/kills.json`.

The focused live check also mutated all four records after an acknowledged cut,
killed the server without saving that mutation, then restored the earlier tick.
It paused another cut before pack sync, revoked the process fence through a
second control connection, and released the worker. Installation returned
`stale tile fence`; HEAD did not change. See `data/E3/coherence.json`.
The same check summoned a pig and dropped stack into nonresident chunks. The
cut request failed and kept HEAD unchanged. Restart then recovered the last
accepted cut. This verifies rejection, not support for capturing that state.

Three Rust tests cover coherent record restoration with later files discarded,
unchanged-page reuse, corrupt-pack rejection before touching the working save,
stale HEAD rejection, stale token rejection and continued monotonic sequence
after a new fence. They passed. These tests and process kills are not a power-loss,
media-corruption, distributed-directory or general item-conservation proof.

## Commands and verification

The retained binary is `.local/bin/horizon-e3/horizon-client`. Reproduce all
measurements without rebuilding:

```sh
python3 -B scripts/e3-cut
```

The sections run separately during implementation were:

```sh
python3 -B scripts/e3-cut --section smoke
python3 -B scripts/e3-cut --section kills --kills 200
python3 -B scripts/e3-cut --section fsync
python3 -B scripts/e3-cut --section bench --samples 20
```

Build and focused checks, from `$REPO/.local/src/horizon`:

```sh
CARGO_INCREMENTAL=0 cargo build --locked --profile fast --no-default-features -p horizon-client --bin horizon-client
E3_TEST_ROOT="$REPO/.local/runs" CARGO_INCREMENTAL=0 cargo test --locked --profile fast --no-default-features -p horizon-client --lib game::dedicated::tile_cut::store::tests
CARGO_INCREMENTAL=0 cargo clippy --locked --profile fast --no-default-features -p horizon-client --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo fmt --all --check
rustfmt --edition 2024 --check --config skip_children=true crates/horizon-client/src/game/dedicated.rs crates/horizon-client/src/game/dedicated/status.rs crates/horizon-client/src/game/dedicated/tile_cut.rs crates/horizon-client/src/game/dedicated/tile_cut/store.rs crates/horizon-client/src/game/persist.rs
```

Build passed. The final warm build reported 1 minute 44 seconds. The three focused
tests passed in 0.17 seconds. All-target strict Clippy passed, final check 14.83
seconds. Whole-workspace formatting failed on the pre-existing E2 input trace
hunk in `lan/hub.rs`. It was not reformatted. Scoped E3 formatting and `git diff
--check` passed. No full Horizon test suite or vanilla parity suite was run.

Checksums and commands are in `data/E3/build.json`; runtime environment files
record retained binary checksums. E3 binary SHA-256:

```text
f797d80094804fb6d86019465ccc0c3cae33d6765d7954362e1cbbfb8238d307
```

## Failures and limits

- The first compile used the wrong fs4 module path. It was fixed to use the same
  standard file-lock API Horizon already uses.
- The first server harness lacked required forwarding and whitelist settings.
  Those runs failed before timing. A population check later caught `/fill` being
  called before chunks loaded. No empty population was accepted as a populated run.
- Four legacy gamerule names were rejected by 26.3. The measurements consequently
  use the defaults, not the intended overrides. Both baseline and cut runs used
  those same defaults. Saved rules and actual populations are retained. No-op
  setup lines were removed from the delivered runner without changing the
  effective measured configuration.
- `Streaming::busy()` was the wrong save barrier and rejected every cut. The
  existing generation save barrier replaced it, with a fail-closed timeout.
- The first normal-terrain validation found 64 live pigs but only 32 in the save.
  The bot's safe spawn was at X=96.5, Z=-31.5, away from the fixture. Ordinary
  entity saving selected only resident chunk keys. The E3 path now scans the full
  live footprint and refuses a cut if any of it is not resident. An unloaded
  pig and dropped stack are explicit rejection regressions. Benchmark bots are
  moved to the fixture before it is populated. Earlier loaded-flat results use
  the superseded binary and are kept under `preliminary-v2-*`.
- An initial statistics/advancement adapter fsynced per guest file. One 50-bot
  populated-flat dry run took roughly 700 to 900 ms. It was replaced by grouped
  in-memory records. Its 143 completed recovery checks and single-sample timing
  results are retained as `preliminary-*`, not pooled into the final results.
- An initial `--bin` test command ran zero tests. The corrected `--lib` command
  ran the three new tests. Only that result counts.
- One second Cargo invocation was cancelled while waiting for the target lock.
  No second compiler ran. All completed builds used the same `fast` target tree.
- This captures Horizon's persisted representation at one boundary, not its
  entire running process. It inherits the existing entity save/load fidelity.
  AI paths, all brain state, per-entity RNG, network sessions, entity numeric IDs,
  tick ordering counters and other transient runtime fields are not preserved
  bit for bit. Scheduled block/fluid ticks and moving pistons use existing chunk
  serializers; general pending block-event, neighbour-update and RNG state is
  not a durable per-tick log.
  Entity kinds without an existing persistence path, including area-effect
  clouds, are not added by this patch. This is not bit-exact process continuation.
- Cuts with live entities or containers outside the resident chunk map are
  refused. The adapter is not a general snapshot of an arbitrary incomplete
  streaming state. This limitation prevents accepting a known incoherent save.
- Native mod callbacks, riding/sleeping/dead players, cross-dimension recovery,
  cursor-overflow cases, active hoppers and redstone, and continuous container
  interaction were not fault-tested. No mods were loaded. Some ordinary metadata
  save helpers swallow their own IO errors; ENOSPC and permission-error safety
  were not established by this experiment.
- The authoritative shared player store is not wired into this patch. Recovery
  does not filter departed player generations against it. Fence tokens here use
  one local directory and file lock, not a distributed ownership service.
- This is a small loaded-tile benchmark with stationary creative bots. It does
  not establish arbitrary tile-size cost, moving-player crossings, survival
  gameplay performance, production garbage collection or a long soak. An 8 ms
  barrier distribution and end-to-end P95 still need direct combined measurement.

Only permitted Horizon MIT source and the allowed project notes were used.
No Folia, Paper, other GPL source or `docs/research/05-folia.md` was read.

## Disk and cleanup

The largest observed `.local` size was 3,113 MiB, about 3.04 GiB. The largest
observed Horizon `target/` was 1,900 MiB. These are observed snapshots, not a
continuous high-water trace; shared `.local` includes the other agents' work.
The largest final benchmark run directory was 224.15 MiB. The final crash run
reached 83.40 MiB.

All E3 world and log directories were removed after their run. Only small metrics,
the measured-driver audit snapshot, the patch and retained binaries remain.
Preliminary JSON is compressed. Redundant generated case files were removed
only after equality with the retained aggregate was checked. Final evidence is
about 932 KiB. Generated Python bytecode was removed too.

```sh
GOLIATH_ROOT="$REPO" scripts/clean-builds --all
```

Cleanup freed 1,847 MiB of Horizon build output. The project was then 725 MiB and
`.local` 713 MiB. The target tree is absent. The delivered runner's live smoke,
unloaded-footprint rejection and fencing tests passed again after target removal,
using the retained binary. Build output is reproducible; temporary run worlds
and logs were deliberately not retained. No commit or push was made by this agent.

## Independent verification

The reviewer reran the 50-bot populated cell and the kill campaign on 2026-11-07 with `python3 -B scripts/e3-cut --section bench --only populated-50 --samples 20` and `--section kills --kills 200`.

| Check | First run | Reproduction |
| --- | ---: | ---: |
| Request P95, cuts 1 s apart, ms | 157.4 | 160.5 |
| Request P95, cuts 5 s apart, ms | 160.2 | 158.6 |
| Tick stall P95, cuts 5 s apart, ms | 89.0 | 94.0 |
| Frozen delta request P95, ms | 169.7 | 173.0 |
| Kill trials passed | 200 of 200 | 200 of 200 |
| Partial loads | 0 | 0 |

The verdict stands. Raw data is in `data/E3/repro-*.json`. E6 replaces this storage design.
