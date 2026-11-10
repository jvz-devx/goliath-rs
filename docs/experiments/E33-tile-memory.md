# E33. Tile memory, huge-page promotion and long plateaus

Status: project stopped. Both builder soaks completed; the reviewer reproduced the THP-disabled soak and adopted the allocator setting in D12. M3 is paused after 234 of 1,000 verified seeds, zero campaign failures; the remaining 766 seeds and matrix are not run. The pilot and both directed restart attempts are reported in [E29](E29-measurements.md#m3-on-final-r2-e29). This experiment does not replace a 30-minute M4 row or claim complete M3.

## Stop state and handoff

`e29-m3-crash-w2` reached its clean pause before automatic continuation was terminated. No further window, cell or build started. The host is restored: window free, `user.slice` effective CPUs `0-5`, no `goliath-e28` cgroup, no run folder and no restore journal. All 29 harness suites pass, 380 tests. No functional script changes were made after `CAPACITY FLAG READY`.

The jobs-2 pilot completed 16 trials and selected concurrency; it is outside the campaign count. Campaign seeds 240001 through 240234 were independently checked with zero duplicates, missing units or stale files. The first store-restart attempt failed in observer setup before either crash; its single declared retry completed lease checks. All four owners survived the first quick restart. The second restart's replacement grant followed a full lease even from the observed listener bound. All clients exited after the intentional tile kill, so continuing player load during replacement remains unproven. Both attempts are retained, not merged into a passing row.

Not run: 766 remaining campaign seeds, the 18-case matrix, full `e24-m3-results`, campaign reviewer reproduction and seven of the eight capacity rechecks. The reviewer ran only plain 200, repeat 0, which passed. No other capacity row is claimed. No commit or push was made by the builder.

Exact later resume, only after authorization, preserving the original schedule and continuing at seed 240235:

```sh
GOLIATH_WINDOW_OWNER=E28T python3 -u -B scripts/e24-m3 campaign --name e29-m3-crash-w3 --data-name e29-m3-crash --window-name e29-m3-crash-w3 --evidence-dir .local/evidence/E29/m3 --bins .local/bin/goliath-e28/final-r2-e29 --cpu-mode pin --seed 240001 --crossings 1000 --jobs 2 --budget-seconds 1200 --resume-from e29-m3-crash-w2
```

See the [paused-prefix archive](data/E29/e29-m3-paused-w2.json.gz) and [final suite counts](data/E33/harness-tests.json). Original trial files remain available for resumption. Do not rerun the fresh automatic driver or replay completed seeds.

## What the retained runs show

The strongest explanation is promotion of mimalloc's anonymous mappings into transparent huge pages. Confidence is high for the mechanism behind the large steps, but retained rollups alone do not prove which kernel event ran or distinguish live payload from unused pages filled during promotion. The first run has no rollups, so its attribution remains an inference from the shape and the two later runs.

All times below are loaded minutes. Broad brackets include the shoulders of each climb rather than claiming exact change points from noisy samples. Memory means resident plus swap. The complete per-minute reductions, endpoint thread names and original-counter values are under [data/E33](data/E33/).

Builder reductions of the historical reviewer run use the `e33-reviewer-soak-r2-*` filenames. They read the existing `repro-e29-soak-r2` evidence; they are not another reproduction or new reviewer measurement.

| Run | Tile | Climb, minutes | Increase, MiB | Anonymous increase, MiB | AnonHugePages increase, MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| e29-soak-r1 | a | 0 to 5 | 72.9 | unavailable | unavailable |
| e29-soak-r1 | b | 18 to 24 | 54.7 | unavailable | unavailable |
| e29-soak-r1 | c, d | No comparable large step by minute 30 | | | |
| e29-soak-r2 | a | 0 to 3 | 62.8 | 61.2 | 158 |
| e29-soak-r2 | b | 10 to 16 | 52.1 | 53.8 | 180 |
| e29-soak-r2 | c | 23 to 25, then 31 to 33 | 10.5, 40.8 | 10.5, 40.8 | 36, 124 |
| e29-soak-r2 | d | No comparable large step by minute 48 | | | |
| repro-e29-soak-r2 | a | 0 to 2, then 42 to 48 | 28.2, 13.1 | 28.2, 13.0 | 108, 34 |
| repro-e29-soak-r2 | b | 2 to 7 | 71.6 | 71.6 | 220 |
| repro-e29-soak-r2 | c | 13 to 18 | 68.4 | 67.7 | 212 |
| repro-e29-soak-r2 | d | 24 to 26, then 31 to 38 | 12.4, 44.8 | 11.9, 45.8 | 54, 92 |

The process counters and rollups are read sequentially. Small endpoint differences between the two are sampling differences, not extra attributed allocations. `Pss_File` is unchanged at the large steps, within a few KiB in the reviewer run. The huge-page increase exceeds the RSS increase because most bytes in the promoted mappings were already resident as ordinary pages. The rise in anonymous RSS is consistent with filling previously untouched portions of those mappings. It is not valid to subtract the whole huge-page increase from RSS and call the result live memory.

Every retained ownership snapshot has 373 chunks and 42 terrain readers per tile. Placed-player counts fluctuate with the crossing cohort rather than stepping permanently. The JSON reductions retain those counts, held players, tick time, TPS and combined page faults at each minute. The old sampler combines minor and major faults, so the two cannot be separated retrospectively.

Thread counts do rise, but not consistently at the large steps. The endpoint name histograms show growth mainly in `horizon-control` client threads, whose Linux names are truncated to 15 characters. The two `horizon-net-*` threads, three worker threads, three chunk-encoding threads, region writer and cut writer are already present and remain the same size. Endpoint names cannot identify the exact thread added between two intermediate samples.

The native tile logs were read as a stream from their retained byte-preserving archives. The extracted relevant lines and crossing counts by destination are in `*-log-correlations.json`. Ordinary log lines are aligned approximately using their preceding input-trace timestamp. Loaded-time saving, checkpoint or compaction messages do not line up with the large steps. Slow-tick bursts around minute 33 of the builder's long run and minute 32 of the reproduction coincide with memory pressure. They are retained rather than excluded. Cut-buffer sizes were not sampled in those runs, and silence in a log is not proof that compaction or an allocation did not happen.

The proxy activation event has no destination field. Destination counts join each actor's activation to its preceding CAS target. The final partial minute is retained and must not be mixed with full-minute rates.

## Pinned source evidence

The checkout is read-only. Its pin is `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. The six files covering the allocator selection, thread implementation, IO pool, cut runtime, cut store and saves match the frozen `build-final.json` source hashes. The final-r2 patches change lease reads and registration, not these paths. Mimalloc's vendored source carries the MIT licence. No external implementation was copied.

File references are relative to `.local/src/horizon-tiles/`:

| Owner | Evidence and implication |
| --- | --- |
| Allocator | `crates/horizon-client/src/main.rs:4` selects `horizon_threads::Allocator` outside explicit system/census builds. `crates/horizon-threads/src/lib.rs:343` selects mimalloc and documents its per-thread heaps. `crates/horizon-threads/Cargo.toml:10` selects mimalloc 0.1.52 and the vendored libmimalloc-sys 0.1.49. |
| Per-thread allocator lifetime | The vendor `build.rs:8` explicitly selects v3. `src/init.c:223` owns thread-local default heaps and line 395 lazily initializes one. Thread teardown collects or abandons its pages at line 627, rather than treating process RSS as a live-object count. Horizon calls `configure_allocator` at `crates/horizon-client/src/entry.rs:15`; `crates/horizon-threads/src/lib.rs:368` sets purge delay to zero. The default one-second purge delay is therefore not an explanation for a step tens of minutes later. |
| THP request and disable | `third_party/libmimalloc-sys/c_src/mimalloc/v3/src/options.c:103` defaults `allow_thp` to 1 on Linux. Its public option documentation is `include/mimalloc.h:483`. `src/prim/unix/prim.c:413` explicitly calls `MADV_HUGEPAGE` for eligible allocations. `src/options.c:637` reads `mimalloc_<option>`, with uppercase fallback in `src/prim/unix/prim.c:848`. `MIMALLOC_ALLOW_THP=0` is therefore the pinned switch. `src/prim/unix/prim.c:224` then disables THP for that process through `PR_SET_THP_DISABLE`; no host setting needs changing. |
| Network pool | `crates/horizon-client/src/lan/io_pool.rs:265` uses `OnceLock` and starts a fixed pool of one thread per two available CPUs. It cannot explain gradual growth from 24 to 30 threads. |
| Control threads | `crates/horizon-client/src/control.rs:267` starts an accept thread and one named client thread per control connection. The endpoint histograms implicate this path in thread growth, without proving why connections remain open. |
| Cut runtime and cache | `crates/horizon-client/src/game/dedicated/tile_cut.rs:87` owns maps of seen/prepared chunks and one writer, created at line 195. Prepared state is retained by the loaded chunk set around line 555. `tile_cut/store.rs:311` holds the current image, not every historic delta. The on-disk log compacts at 64 MiB at line 410. Encoding and compaction allocate temporary vectors; a 64 MiB log threshold is not a 64 MiB resident-cache limit. |
| Region saves | `crates/horizon-client/src/save.rs:2057` caps each region-file cache at 256 entries, and line 2893 creates a region writer. Line 2239 creates scoped encoding helpers. These can cause transient allocation or allocator retention, but do not explain a rise in huge-page coverage with unchanged chunks by themselves. |
| Player autosaves | `crates/horizon-client/src/lan/playerdata.rs:286` lazily starts one `horizon-player-writer` on its first enqueue, at line 297. This accounts for the single late player-writer name in the endpoint histograms; it is distinct from the multiple additional control-client threads. It can explain a small new heap, not the fixed IO pool growing. |
| Measurement buffers | `crates/horizon-client/src/game/e11_measure.rs:11` caps the tick/digest queues at 8,192 entries and line 140 drains them. The soak drains once per minute, well below this cap. An unbounded tile measurement queue is not supported by this source. |

### Ranked hypotheses and discriminating observations

1. **THP promotion of allocator mappings.** Supported by every large measured step in both extended runs, stable file-backed memory and asynchronous timing across tiles. A tile-only THP-disabled run with `THP_enabled=0`, no `AnonHugePages` and no comparable steps tests the cause. Comparable steps with THP disabled would refute promotion as the sole explanation.
2. **Per-thread allocator retention or persistent control connections.** Plausible for smaller shoulders and slow growth, since control-thread counts rise. A memory rise at a new thread without a THP rise supports this. Large steps with unchanged threads already argue against it as the main cause.
3. **Cut image, save-buffer or region-cache high-water marks.** Real allocation sites, but no observed growth in the loaded chunk set. The new sampler records cut LOG/BASE sizes without issuing a cut. Correlated cut sizes without THP growth would support this; flat cut sizes across a step weaken it. A fixed chunk count does not itself bound all temporary encoded bytes.
4. **Measurement buffers or a growing world.** Low confidence. Buffers are bounded and drained, and chunks/readers stay fixed. Dropped samples, rising chunk counts or growing queue occupancy would revive this hypothesis.

## Harness retention fixed before new soaks

The dominant known retention path is `.local/src/horizon/tools/server_bench.py:133`. Its reader appends every tile stdout line to `self.lines` and also writes it to the native log. Millions of input-trace strings accumulate for the whole run. This accounts for GiB-scale retention without requiring a growing collection of status rows.

The Linux adapter now uses disk-backed console history. It writes the same lines with the same flush behaviour and keeps one seek cookie per 1,024 lines. `count()` still returns an absolute line count; `since(n)` reads the identical requested history from disk, including after close. The test checks 10,000 Unicode lines, marks on either side of a block boundary, negative slicing, exact file contents and bounded index size. No native line is dropped. Existing measurement rows remain recorded.

New reports publish the observer/driver's resident and swapped memory every minute. This checks what the fix actually achieved; the source diagnosis alone is not a claim that every remaining retained collection is bounded.

## New measurement protocol

The first new soak is `e29-soak-r4-diag-thp`, using `MIMALLOC_ALLOW_THP=0` only in tile environments. It retains the standard 425-player fixture, final-r2-e29 binaries, pinned CPU budget, direct bots, receipt-window baseline and 25-minute judged window. It is labelled diagnostic and does not count as an M4 row. The host, proxy and store keep their normal THP behaviour. Tick time and process CPU will be compared with `e29-soak-r2`, with the changed harness memory pressure reported as a confounder.

Next, `e29-soak-r3-long` uses the host's normal settings for 120 loaded minutes. Bots live for 10,800 seconds. It reports every process, all climbs and last-60/30-minute growth. The unchanged ruled memory interval is published separately, without a new verdict on the M4 line.

For every new window the policy record contains `enabled`, `defrag` and every readable `khugepaged/*` value before and after. The per-minute extended sampler records RSS, PSS, anonymous memory, huge pages, swap, file-backed RSS, VmData, VmHWM, thread-name counts, separate minor/major faults, process `THP_enabled`, global THP counters and cut file sizes. Existing ownership audits supply chunks, placed/held players, readers and tick counters without another control request.

## Tile-only THP-disabled diagnostic

`e29-soak-r4-diag-thp` completed once, with 2,881.3 loaded seconds. The store's receipt window selected minute 23 as the baseline. There was no retry, collection error, authority stop or clock anomaly. All four tiles report `THP_enabled=0` and `AnonHugePages=0` at every one of the 49 minute probes. No large climb occurs.

| Tile | Loaded start, MiB | Loaded end, MiB | Full sampled range, MiB | Ruled-window change |
| --- | ---: | ---: | ---: | ---: |
| a | 242.17 | 249.32 | 242.11 to 251.15 | +0.73% |
| b | 240.41 | 246.29 | 237.83 to 249.13 | +0.21% |
| c | 239.78 | 246.03 | 239.56 to 249.58 | -0.17% |
| d | 239.88 | 247.90 | 238.75 to 249.48 | +0.37% |

This is the expected intervention result for the promotion hypothesis. Combined with the retained smaps correlations and the allocator's explicit advice, it identifies THP as the cause of the earlier large steps. It does not prove the absence of smaller genuine allocation growth. The normal-setting long run will test recurrence under the same fixed harness.

### Tick and CPU comparison

Process CPU is the whole loaded interval's cumulative process CPU delta divided by elapsed time. Mean tick and raw P99 use all retained raw ticks, without removing swapping bursts or tails.

| Tile | e29-soak-r2 CPU, cores | THP off CPU, cores | r2 mean tick, ms | THP off mean tick, ms | r2 raw P99, ms | THP off raw P99, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 0.487 | 0.501 | 22.04 | 23.07 | 43.66 | 42.22 |
| b | 0.501 | 0.496 | 25.49 | 22.23 | 43.73 | 38.76 |
| c | 0.495 | 0.498 | 24.66 | 21.77 | 42.94 | 41.25 |
| d | 0.503 | 0.501 | 24.11 | 23.60 | 43.84 | 41.72 |

There is no consistent tile-CPU change. These are not isolated THP speed measurements. The old run's cell driver reached 5,067 MiB and caused swapping; this run's cell driver rose from 64.7 to 600.1 MiB with no driver swap. Its resident series is in [the minute report](data/E33/e29-soak-r4-diag-thp-minute-report.json.gz). The separate window coordinator reaches 404.66 MiB, so the two harness processes together use about 1,005 MiB at minute 48. Other retained collections still grow, about 11 MiB per loaded minute in the cell driver, so the fix removes the dominant retention rather than claiming constant harness memory. Host swap-out is zero over this window, versus the old run's swapping bursts. New minute probes take at most 363.5 ms of wall time across five processes, once a minute; their raw durations are retained.

### Other diagnostic lines

| Line | Observation |
| --- | --- |
| Online players | 425 in all 1,633 samples |
| Owner audits | 47, zero violations and generation regressions; crossing-in-progress brackets remain unresolved |
| Cargo after checkpoint and recovery | 42 of 42, none duplicated or missing |
| Crossings per full minute | 375 to 384 |
| Game-time TPS | 20.000 on every tile, within the measurement rounding |
| Raw tick samples | 57,673 to 57,677 per tile, zero dropped |
| Store barriers | 17,938; over 50/100/200/400 ms: 100/25/3/1; maximum 408.85 ms |
| Renewals | 46,053 sent, all answered early, none journaled or unmatched; slowest tile reply 20.57 ms |
| Early answer inside store | Maximum receipt-to-answer 20.09 ms; maximum accept-to-receipt 2.71 ms |
| Whole window | 3,285.33 s, D12 valid, CPU 5 72.60% busy, largest outside process 0.140 core, exact policy restoration |
| Host THP settings | `madvise` before and after; no settings changed |

The barrier exceeds 400 ms but is shorter than the 463 ms barrier in the earlier failed soak and does not reproduce its pair of consecutive barriers. No barrier-mode causal control was run. This row is a THP diagnostic, not an M4 verdict.

Data includes the [result](data/E33/e29-soak-r4-diag-thp-e25-result.json), [store and renewal readback](data/E33/e29-soak-r4-diag-thp-store-renewals.json), [all memory samples](data/E33/e29-soak-r4-diag-thp-memory-series.json.gz), [audits and detailed probes](data/E33/e29-soak-r4-diag-thp-audits.json.gz) and [per-minute reduction](data/E33/e29-soak-r4-diag-thp-minute-report.json.gz). Native traces stay under `.local/evidence/E33/soak/`. The run directory was deleted after capture.

## Normal-setting 120-minute soak

`e29-soak-r3-long` ran once for 7,200.81 loaded seconds, with 121 detailed minute probes. `THP_enabled=1` on every tile throughout. The host remained in `madvise` mode. All traces are framed, all binaries unchanged, and no authority-stop or clock-anomaly event occurred.

The answer is a bounded range over these two hours, not one irreversible step followed by a perfectly flat line. Every tile makes its large initial climb. Smaller later rises and falls remain visible, especially tile a around minutes 50 and 100, tile b near 60 and 110, and tile d near 88. Their endpoint net changes over the last hour are small. That does not erase the intermediate movements or establish an indefinite leak bound.

### Every process and both tail windows

All figures below are resident plus swap in MiB. Every listed process has zero swap at the final endpoint. Growth uses the nearest full-rate sample to the exact tail-window start; the displayed minute samples are rounded independently. The [minute report](data/E33/e29-soak-r3-long-minute-report.json.gz) includes every minute, detailed probes, all helper processes and a separate driver resident/swap series.

| Process | Minute 0 | Minute 60 | Minute 90 | Minute 120 | Last 60 min | Last 30 min |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Proxy | 310.02 | 313.32 | 314.14 | 314.83 | +0.484% | +0.221% |
| Store | 6.96 | 106.39 | 106.41 | 106.51 | +0.114% | +0.092% |
| Tile a | 288.05 | 368.11 | 358.00 | 358.31 | -2.661% | +0.088% |
| Tile b | 281.48 | 353.60 | 341.79 | 346.83 | -1.914% | +1.477% |
| Tile c | 279.61 | 348.88 | 348.54 | 349.11 | +0.066% | +0.163% |
| Tile d | 277.62 | 351.45 | 363.82 | 362.01 | +3.004% | -0.499% |
| Sidecar 0 | 3.29 | 3.38 | 3.39 | 3.39 | +0.347% | +0.000% |
| Sidecar 1 | 3.23 | 3.33 | 3.33 | 3.41 | +2.345% | +2.345% |
| Sidecar 2 | 3.23 | 3.33 | 3.33 | 3.34 | +0.352% | +0.352% |
| Sidecar 3 | 3.22 | 3.34 | 3.36 | 3.37 | +0.701% | +0.349% |
| Cell driver and observer | 64.04 | 750.37 | 1,078.45 | 1,405.64 | +87.327% | +30.339% |
| Window coordinator | 42.34 | 515.17 | 743.96 | 976.02 | +89.456% | +31.193% |
| Bots a | 10.61 | 10.61 | 10.61 | 10.61 | +0.000% | +0.000% |
| Bots b | 10.47 | 10.46 | 10.46 | 10.46 | +0.000% | +0.000% |
| Bots c | 10.61 | 10.61 | 10.61 | 10.61 | +0.000% | +0.000% |
| Bots d | 10.58 | 10.56 | 10.56 | 10.56 | +0.000% | +0.000% |
| Crossing bots | 6.81 | 6.96 | 6.98 | 7.02 | +0.898% | +0.616% |

The store's receipt-window fill is its initial growth, not another late step. It then stays near 106.5 MiB. The cell driver still retains ordinary result collections and rises at about 11.2 MiB per loaded minute, rather than the previous 100 MiB per minute. The window coordinator retains its own whole-host samples too. Together the two harness processes reach about 2.33 GiB at minute 120, with zero endpoint swap in both. This fits the two-hour run but is not constant-memory collection. Both harness processes' resident/swap series for every historical and new run are in [the harness report](data/E33/harness-memory-all-runs.json), and the original all-process minute files retain their PIDs and sampling times.

Small endpoint growth is not the full sampled range. Across all full-rate samples in the last hour, tile a ranges from 355.58 to 374.15 MiB, b from 337.30 to 363.81, c from 342.35 to 360.28, and d from 347.34 to 378.77. Relative to its minute-60 baseline, d's intermediate high is +7.77%, although its endpoint is +3.004%. The other high deviations are +1.64%, +2.89% and +3.27%. Both the increases and decreases are retained in [the range report](data/E33/tail-ranges-and-cuts.json). "Within about 3% over the last hour" describes endpoint growth only.

For completeness, the unchanged ruled interval is minutes 23 to 48. Its numeric growth is proxy +0.092%, store +4.762%, tiles a to d +2.256% / -1.743% / +7.861% / +24.851%, and sidecars +0.117% to +0.698%. Tile d's main promotion falls inside that interval. These figures are retained separately from the late-window plateau analysis. This labelled long row does not replace a 30-minute M4 row or introduce a new ruling.

### All qualifying rises

To avoid choosing only convenient endpoints, the report finds every three-minute interval whose memory rises at least 10 MiB, then groups qualifying starting minutes separated by at most two minutes. The full detector output is [retained](data/E33/e29-soak-r3-long-climbs.json). This is a reproducible rise detector, not a claim that each row is a discrete allocation or leak. Reversible humps count too.

| Tile | Broad interval, minutes | Memory rise, MiB | Anonymous rise, MiB | AnonHugePages rise, MiB |
| --- | ---: | ---: | ---: | ---: |
| a | 0 to 5 | 48.03 | 47.72 | 114 |
| a | 49 to 55 | 28.46 | 27.71 | 66 |
| a | 95 to 100 | 12.56 | 13.73 | 36 |
| b | 9 to 16 | 65.70 | 67.66 | 198 |
| b | 57 to 61 | 12.84 | 10.77 | 24 |
| b | 106 to 110 | 17.91 | 20.54 | 24 |
| c | 18 to 27 | 58.78 | 58.78 | 206 |
| c | 73 to 76 | 12.29 | 9.84 | 38 |
| d | 19 to 22 | 10.02 | 8.63 | 6 |
| d | 29 to 43 | 80.54 | 78.39 | 194 |
| d | 84 to 88 | 20.89 | 20.92 | 28 |

Sequential memory and rollup reads explain the smaller endpoint differences. Most substantial rises again accompany increased huge-page coverage. Tile d's small early hump has only 6 MiB more huge pages; these data do not attribute every byte of every smaller movement to promotion. No other production process has a qualifying late rise. The driver has a continuing slope, not a single step.

File-backed PSS changes by at most 4 KiB across the reported tile intervals. Several later rises have exactly unchanged thread counts, including a at minutes 95 to 100, b at 57 to 61 and 106 to 110, and d at 84 to 88. That argues against a new thread alone causing those movements. The data retain separate fault counts and thread names, but do not assign individual faults or allocations to a particular kernel promotion.

Cut-log rotations are frequent throughout both runs, rather than a one-time event aligned with a large climb. Each THP-disabled tile has at least 21 sampled LOG falls of more than 1 MiB in 48 minutes. The normal run has 52, 52, 53 and 52 over 120 minutes. LOG peaks near 64 MiB in both runs, and the final BASE is about 0.68 MiB per tile. Minute samples give a lower bound, not an exact count of rotations. This weakens the first-compaction explanation: the disabled run still rotates the log repeatedly without a 40-to-80 MiB resident-memory step. It does not rule out transient compaction buffers or allocator reuse as part of the smaller movements.

### Usual long-soak observations and the bot flag

| Line | Observation |
| --- | --- |
| Players | 425 in all 3,664 online samples |
| Authority and clock events | None, complete traces |
| Owner audits | 118, zero violations or generation regressions; changing query brackets remain unresolved |
| Cargo after checkpoint and recovery | 42 of 42, no duplicates or missing items |
| Crossings per full minute | 374 to 382 |
| TPS, tiles a to d | 20.000 / 19.994 / 20.000 / 20.000 |
| Long-window raw tick P99, tiles a to d | 40.00 / 41.00 / 40.14 / 40.77 ms |
| Mean tick, tiles a to d | 21.55 / 21.95 / 22.80 / 22.71 ms |
| Tile process CPU, a to d | 0.484 / 0.488 / 0.495 / 0.490 core |
| Store barriers | 43,873; over 50/100/200/400 ms: 258/51/17/3; maximum 539.98 ms |
| Renewals | 115,055, every one answered early, none journaled or unmatched; slowest tile reply 25.29 ms |
| Early store timing | Maximum receipt-to-answer 23.42 ms; accept-to-receipt 3.70 ms |
| Whole D12 window | 8,075.41 s, valid and restored; CPU 5 74.78% busy, largest outside process 0.13664 core |
| Host swap over the whole window | 18,900 pages in, 79,068 out; this includes startup and archival |
| Capacity diagnostic flag | Failed and retained, one late bot round, no retry |

Cohort c reports its single late round at loaded second 2,154.26, about minute 35.9, with maximum lateness 54.80 ms. All clients stayed online, and total bot CPU was 0.281 core. Its cause is unproven. The row is not presented as capacity-qualified. The memory and two-hour availability observations remain reported as a labelled long-soak result, not an M4 replacement.

The 540 ms barrier is longer than those in the previous E29 soaks, and the tiles continue renewing. This is useful availability evidence. It is still neither a barrier-mode causal control nor the exact earlier pair of consecutive slow barriers.

Data includes the [full result](data/E33/e29-soak-r3-long-e25-result.json), [store/renewal readback](data/E33/e29-soak-r3-long-store-renewals.json), [memory samples](data/E33/e29-soak-r3-long-memory-series.json.gz), [audits and detailed probes](data/E33/e29-soak-r3-long-audits.json.gz) and [every-process minute report](data/E33/e29-soak-r3-long-minute-report.json.gz). Native traces remain under `.local/evidence/E33/soak/`. The run directory was removed after capture.

## Measurement decision and remaining limits

The reviewer adopted `MIMALLOC_ALLOW_THP=0` on Linux Horizon processes on both sides as a D12 addendum after reproducing r4. Reported ruled growth in that reproduction is tiles +0.5% to +1.1%, store +5.1%, proxy -0.1%. The builder did not edit the decision file. Historical normal-THP rows remain historical rows, not retroactively relabelled results.

- Adopted: THP off is the D12 condition on Horizon in both the plain and cluster fixtures. This controls allocator/kernel policy but changes the measured platform condition and may change speed. Old rows keep their original results and settings. They cannot be relabelled as THP-off rows; matched capacity rechecks are needed before comparing frontiers. Only one recheck ran before the stop.
- Not implemented: a promotion-insensitive measure such as directly accounted live allocations, with an explicit policy for non-heap memory. That would change what the line tests and needs a reliable new counter. RSS, PSS, anonymous RSS and resident-plus-swap all include promotion's effect. Subtracting `AnonHugePages` is not live-allocation accounting. Old rollups cannot reconstruct the missing allocation counter.

The builder did not edit progress or decisions. The documented limits remain: later reversible movements exist, one late bot round remains, the two harness processes still retain growing result collections, capacity rechecking stopped after one cell, and M3 is incomplete. A promotion-insensitive allocation counter was not implemented.

## Reproduction commands

Pending reviewer reproduction. Create the equivalent registration under the new name before starting. The runner takes and releases its own window; readback runs only after release.

```sh
export GOLIATH_WINDOW_OWNER=E28R
python3 -u -B scripts/e25-soak-control --name repro-e29-soak-r4-diag-thp --bins .local/bin/goliath-e28/final-r2-e29 --data-dir docs/experiments/data/E33 --cpu-mode pin --direct-bots --players 425 --extended-memory-window --tile-thp-disabled --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
python3 -B scripts/e25-soak-readback --name repro-e29-soak-r4-diag-thp --data-dir docs/experiments/data/E33 --owner E28R
python3 -B scripts/e29-soak-report --name repro-e29-soak-r4-diag-thp --data-dir docs/experiments/data/E33
python3 -u -B scripts/e25-soak-control --name repro-e29-soak-r3-long --bins .local/bin/goliath-e28/final-r2-e29 --data-dir docs/experiments/data/E33 --cpu-mode pin --direct-bots --players 425 --extended-memory-window --plateau-minutes 120 --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
python3 -B scripts/e25-soak-readback --name repro-e29-soak-r3-long --data-dir docs/experiments/data/E33 --owner E28R
python3 -B scripts/e29-soak-report --name repro-e29-soak-r3-long --data-dir docs/experiments/data/E33
```
