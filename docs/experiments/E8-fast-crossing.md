# E8. Faster wall crossings

2026-11-07. Implementation evidence, pending review.

## Result

The final configuration removes both client arrival corrections, queues input
through the lane change, and recovers automatic routing after a real death and
respawn. The 80 ms handoff budget is not met. This experiment does not include
the D6 tile checkpoint or shared-store commit.

The final rebuilt proxy completed 1,000 of 1,000 crossings. Input gap
P50/P95/P99 was 129.440 / 145.712 / 182.129 ms, with zero disconnects, zero
inventory changes and zero client arrival teleports. Both the largest backward
correction and the largest absolute arrival correction were zero blocks.
Real death/respawn, E4's unchanged edge probes, turn-back/expiry cleanup and the
live block-sequence probe passed.

The bare P95 is below 150 ms, but reserving the requested 40 ms checkpoint and
4 ms store commit would leave a budget illustration of 189.712 ms. This is not
a measured D6 crossing. Even export-request-to-first-input P95 is 129.795 ms,
so the path does not fit an 80 ms share by either timing definition. M2 remains
open.

An earlier separate 1,000-crossing stage-4 run had P95 159.086 ms overall,
178.539 ms on the route and 144.116 ms on straight passes. It is retained in
`stage4.json`, not pooled with or discarded in favor of the final rebuild.
The next 1,000-crossing rebuild had P95 146.954 ms and is retained in
`stage4-before-bookkeeping-fix.json`. A final source check caught unused legacy
teleport ids accumulating alongside the new mapper. That bookkeeping was
removed, a regression test added, and the complete final 1,000 run repeated.

These are kinematic protocol clients at 5.6 blocks/s, with a nominal 50 ms input
cadence. They do not establish physical momentum. Horizontal server velocity
after arrival is zero in the recorded trials. The proxy carries the frozen
record's server velocity and does not reset the client's pose or velocity with
an arrival teleport, but a physics client must still verify the result.

## What changed, in order

1. A dead-player refusal no longer permanently disables automatic routing.
   The player stays on the source. Later movement retries after a one-second
   cooldown, including after respawn. Other refusals retain E4's stop behavior.
   The real-death probe uses the server's `kill`, observes the real refusal,
   requests respawn through the protocol, crosses to b, and returns to a.
   `keep_inventory` keeps the known inventory for this probe.
2. Approach preparation uses the last movement direction to predict a point
   four blocks ahead. One wall worker owns one held login and view. It waits
   for activation at the first eligible crossing, while later movement remains
   the deferred destination. A turn away before crossing cancels preparation.
   An unused ready preparation expires after three seconds. Login/configuration
   has an eight-second timeout. No background preparation handle survives its
   worker or keeps the session alive through a reference cycle.
3. A queue starts before the freeze request. It holds at most 256 packets and
   1 MiB of payload. Overflow closes the session instead of silently discarding
   an action. Replay preserves packet order and applies the target's existing
   entity-id rewrite only when the destination is final. An aborted handoff
   replays to the resumed source. In this intermediate configuration replay
   waits for source release, which adds delay rather than removing it.
4. The target import uses the existing `place:true` mode. It writes the player's
   files durably and places the player in that request. A separate confirm RPC
   is unnecessary. Both target position corrections are acknowledged on the
   passive lane. The synthetic delta position is omitted. Input replays after
   durable target placement, before source release. A known committed arrival
   is not aborted back to the source if subsequent client delivery fails.
5. The final run records export, import, placement, release and first-input
   intervals. Tick-bound RPC time and the target's next accepted input remain
   much larger than local packet-rewrite work.

E4 already prepared the target before freezing. Starting that work on approach
mainly shortens wall-trigger-to-arrival time. It does not remove that whole
preparation duration from E4's input gap.

### Readiness and its limit

Initial readiness still requires Login, position, a chunk batch and the 200 ms
settle period, before freezing. After activation, the proxy checks the predicted
arrival's buffered columns. A recentered import can skip the second 200 ms wait
when its actual 3 by 3 arrival ring is present. Missing columns keep the original
bounded wait; the proxy does not infer readiness merely from silence.

Horizon still places a held login at its saved position or spawn. Its import
requires a real frozen handover id, so a fabricated pre-import on approach would
not be a safe way to reposition it. The experiment proves a buffered arrival
ring at these nearby walls, not a positioned observer view or complete terrain
composition past arbitrary distant walls. M2's visible terrain requirement is
not claimed.

The recorded import-reply-to-delta-start interval shows what this removes:

| Configuration | P50, ms | P95, ms | P99, ms | Maximum, ms |
| --- | ---: | ---: | ---: | ---: |
| 1, old settle rule | 0.077 | 0.171 | 3.037 | 283.369 |
| 2, buffered-ring proof | 0.072 | 0.143 | 0.381 | 14.712 |

This interval includes view readiness and session-lock work. Most old-rule
crossings already needed no recenter wait, so removing the cold recenter waits
does not move its P95 much. All 1,005 step-2 predicted-ring checks were true,
including the separate probes; the 1,000 counted intervals retain every sample.

### Lane identities

Keep-alive, ping and teleport challenges get session-wide proxy ids. Each answer
returns to the lane that issued the challenge, with that lane's original id.
An unknown, duplicate or expired answer cannot fall through to the new backend.
Chunk-batch answers also remain bound to their issuing lane.

Block actions get lane-local sequence numbers. The proxy maps cumulative backend
acknowledgements back to the client's sequence only when all earlier actions
have been acknowledged, including actions on the retired lane. Each challenge
map is bounded at 64 entries; pending block actions are bounded at 256.
The old teleport tracker is disabled when the new lane-aware tracker owns
acknowledgements. A regression test prevents duplicate legacy bookkeeping.

Unit tests use colliding ids from two lanes and reordered block acknowledgements.
A live probe sends use-item sequence 37 before crossing, then 38 and 39 after the
freeze request, and receives acknowledgement 39 without an inventory change.
Queue-overflow behavior is unit-tested, not forced against a real engine.
This does not add a global entity-id map or container-state translation.

## Measurements

Each significant configuration has its own walking samples. The counted route
uses E4's full 14-move itinerary for 490 crossings, then 510 straight passes
between x = -3 and x = 3 at z = 1. Each straight leg continues past the wall to
its endpoint before reversing. It is not a short waypoint at the boundary or
one infinite positive-X trajectory through an unbounded tile grid.

Input gap is unchanged from E4: first target accepted movement timestamp minus
last source accepted movement timestamp from `E2_TRACE_INPUTS`. It includes the
input cadence and backend scheduling, not just proxy CPU. Total starts at the
first eligible client position and ends at the successful wall-arrival message.
Approach preparation can therefore precede total's start.

Arrival correction uses E4's window through 100 ms after the first target input.
Backward correction is its horizontal projection opposite the leg's direction.
No sample is trimmed. Percentiles use nearest rank without interpolation.

The rows are cumulative, so each row is the previous row's configuration plus
the named change. Each has 1,000 completed samples and zero gameplay failures.
All times are milliseconds. Triples are P50 / P95 / P99.

| Configuration | Input gap | Total | Largest backward correction, blocks | Horizontal arrival speed, blocks/tick |
| --- | ---: | ---: | ---: | ---: |
| 1, refusal recovery; E4 handoff | 213.355 / 249.991 / 303.453 | 711.545 / 792.372 / 845.317 | 2.360 | 0 |
| 2, approach preparation and ring proof | 200.955 / 250.932 / 252.709 | 197.054 / 1856.772 / 1916.314 | 1.680 | 0 |
| 3, bounded replay and lane ids | 228.707 / 244.430 / 291.303 | 193.842 / 1870.323 / 1923.158 | 1.960 | 0 |
| 4, continuous durable arrival, final rebuild | 129.440 / 145.712 / 182.129 | 149.451 / 446.210 / 468.440 | 0 | 0 |

Before step 1, E4's dead-player refusal stopped automatic routing for the
session. The new real-death probe recovers without reconnecting. For latency,
E4's historical route-only gap was 177.100 / 247.261 / 251.105 ms; it is not a
matched-load baseline for the mixed E8 route and straight passes.

Approach preparation does not improve the gap tail here. Stale directional
predictions that must be abandoned can also delay a new preparation behind the
one-second retry cooldown, explaining the long total-time tail in steps 2 and
3. The player remains active on the source during that delay. Step 3's ordered
replay still waits until source release. Step 4 removes that wait and the
separate confirm request, while eliminating the corrections.
The prediction/cooldown explanation is source-based, not a separately timed
attribution of every slow sample.

| Step and path | n | Input gap, P50 / P95 / P99, ms | Total, P50 / P95 / P99, ms |
| --- | ---: | ---: | ---: |
| 1, route | 490 | 203.929 / 253.550 / 332.316 | 718.929 / 806.306 / 909.360 |
| 1, straight | 510 | 213.723 / 236.282 / 285.921 | 706.533 / 785.979 / 801.753 |
| 2, route | 490 | 200.932 / 248.984 / 256.003 | 473.535 / 1885.813 / 1940.111 |
| 2, straight | 510 | 200.955 / 251.010 / 251.335 | 172.843 / 199.414 / 202.856 |
| 3, route | 490 | 228.707 / 265.230 / 315.201 | 491.297 / 1901.196 / 1964.129 |
| 3, straight | 510 | 226.967 / 230.649 / 269.635 | 158.726 / 199.064 / 1054.207 |
| 4, route | 490 | 129.439 / 145.867 / 184.213 | 371.399 / 456.303 / 477.437 |
| 4, straight | 510 | 130.368 / 132.326 / 132.678 | 104.003 / 146.954 / 150.919 |

Input gap can exceed total. Its last accepted source input can precede the
eligible trigger, and its first accepted target input can follow the arrival
message. The two intervals have different endpoints.

Every successful counted crossing exports inventory before and after. The known
inventory is five diamonds and twelve oak logs, 17 items, with checksum:

```text
c200b92c38f59972717cc23bc2460c8b571c7864080f16e01454c6b3751a8e2f
```

Velocity vectors, accepted-input ticks and positions, both inventory checksums,
teleports, actual arrival order and per-crossing load average remain in the raw
results. Healthy checksum equality does not establish crash safety or unique
authority over stale player files.

### Load and comparison limits

| Configuration | One-minute load average, minimum to maximum during crossings |
| --- | ---: |
| 1 | 21.897 to 42.858 |
| 2 | 13.133 to 37.276 |
| 3 | 23.747 to 42.858 |
| 4, final | 9.913 to 28.085 |

This was not an exclusive-machine benchmark. The long runs overlapped other E8
configurations and other agents' Horizon builds. Pilots and implementations were
started in step order, but the full measurement runs overlapped. Load average is
recorded with every crossing and build. Treat percentile differences as observed
results under that load, not isolated causal estimates or confidence intervals.
The reproduction command runs the four configurations sequentially.

## Remaining tick-bound work

All 1,000 final samples have phase records.

| Interval | P50, ms | P95, ms | P99, ms |
| --- | ---: | ---: | ---: |
| Source export request/reply | 26.390 | 48.853 | 50.453 |
| Target import and durable placement request/reply | 51.147 | 66.476 | 72.148 |
| Target import reply to first accepted input | 26.087 | 28.930 | 29.952 |
| Export request to target placed | 75.938 | 104.828 | 111.095 |
| Export request to first accepted target input | 99.492 | 129.795 | 134.513 |
| Source release, outside the final input queue window | 29.330 | 45.866 | 49.825 |

The separate target confirm in step 3 cost 72.693 / 88.021 / 110.182 ms.
Its import-reply-to-first-input interval was 148.339 / 150.093 / 198.246 ms.
The final path reduces that latter interval's P95 by 121.163 ms. Source export
still shows a roughly 0 to 50 ms tick-phase wait opportunity, and the final
target acceptance follows its import reply about half a tick later.

These are measured request/reply intervals. Export includes source tick dispatch
and serialization. Import includes target tick dispatch and its durable player
file writes. Reply-to-first-input includes network delivery, proxy scheduling
and waiting for the target to consume the acknowledgements and replayed input.
They are not an exact separation of tick waiting from disk work. Pure per-side
tick wait needs server-side queued/applied timestamps; that part is unverified.
Percentiles of components must not be added as if they were one sample.

## Horizon request list

No Horizon checkout was changed or built by E8. The requests below identify work
for its owners. A measured interval is a ceiling on possible saving, not a
measurement of an implementation that does not exist yet.

| Request or existing option | Exact behavior needed | Measured cost or saving |
| --- | --- | --- |
| No new API for combined import/confirm | Keep using `player-import` with `place:true` and its durable placement reply | Already implemented. Removes the separate step-3 confirm RPC with P95 88.021 ms. Whole-path gap P95 changed from 244.430 to 145.712 ms, a 98.718 ms observed difference under different load. |
| Positioned `player-expect` | Accept position, rotation and dimension for the held login's initial chunk tracker, without importing a frozen record or placing the player | No measured extra frozen-window saving on an already covered E8 ring. It would address distant preparation and avoid the existing 200 ms fallback when new chunks are needed; that saving was not measured here. |
| Ordered initial inputs in durable activation | Accept a bounded ordered input batch with the import, apply it only after durable placement, in that same tick, without an outstanding arrival teleport | Current import-reply-to-first-input P95 is 28.930 ms. That is the measured upper bound on this interval's possible saving, not proof that a new API would save all of it. |
| Source and target tick-phase telemetry | Return simulation tick plus queued, applied and durable timestamps for export/import; expose a tick-phase signal for scheduling a freeze/checkpoint request | Source export P95 is 48.853 ms and target import P95 is 66.476 ms. Both include work besides dispatch, so exact tick-alignment savings cannot yet be assigned. |
| D6 authority integration | Make target activation acknowledge the coherent durable checkpoint and fenced store generation, rather than treating three independent backend player files as global authority | No measured saving. Target import includes its ordinary file writes, but E8 does not isolate their cost or establish that removing them is safe. |

Do not remove durability merely to reach the timing target. The existing target
player-file commit is not a coherent tile checkpoint and is not the fenced D6
store commit. Lost commit replies, crashes, partitions, target failures after
placement and source-release failure still need that authority protocol. E8's
healthy runs do not validate those cases.

## Four tile clocks

Four fresh engines ran for 300.130 seconds, sampled about every ten seconds.
Engine a had 128 `horizon-bots` using `mixed`, view distance 3, creative mode and
0.02-second join spacing. The last sample had all 128 online, zero bot failures,
204,739 received chunks and 122,403 unique chunks. Empty-server pausing was
disabled on all four engines. The clock run's load-average range was 30.499 to
42.629 on this 18-core Mac.

Minecraft 26.3 no longer has the old `time query daytime` literal in its prepared
command tree. E8 records `time query gametime` and `time query time`, with the
latter's absolute clock value reduced modulo 24,000 for the daytime column.
Each query retains its command, absolute value and send/receive timestamps.
Queries run concurrently across engines, but execution remains tick-quantized.

| Engine | Game time start | End | Advance | Daytime start | End | Advance |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a, loaded | 39 | 5,999 | 5,960 | 40 | 6,000 | 5,960 |
| b | 27 | 6,027 | 6,000 | 28 | 6,028 | 6,000 |
| c | 14 | 6,014 | 6,000 | 15 | 6,015 | 6,000 |
| d | 1 | 6,002 | 6,001 | 2 | 6,003 | 6,001 |

Sequential startup explains the initial offsets. Removing each engine's initial
value, maximum incremental drift was 72 ticks at 232.524 seconds, or 3.6 seconds
of game time. Final incremental drift was 41 ticks. The largest simultaneously
observed absolute daytime spread was 59 ticks, or 2.95 seconds at 20 TPS.
That permits a sky-phase step at a wall. It is 0.246% of a normal 24,000-tick
day cycle, not a measured rendered angle. No actual stock-client sky jump was
observed. Independent tile clocks are not a
shared clock; choosing a global clock owner still needs a project decision.

## Failures kept

| Run | Completed crossing samples | Outcome |
| --- | ---: | --- |
| `stage1-failed-disk-sampling.json` | 551 of 1,000 requested | Runner exit 2, disk sampling raced directory deletion |
| `stage2-failed-disk-sampling.json` | 576 of 1,000 requested | Runner exit 2, same disk-sampling defect |
| `stage2-pilot.json` | 0 of 28 requested | Proxy `--check` killed by SIGKILL before login |
| `clock-pilot.json` | No crossing trials | Clock-reply parser expected the wrong text and timed out |

The replacement 1,000-crossing runs are separate files. Expected dead-player
refusals are retained in each death probe, with no invented zero-time gap for
the refused attempt. Successful pilots and the earlier 1,000-crossing stage-4
run remain separate too. The first 14-crossing pilot used longer x = +/-8 legs
and reached a 3.64-block backward correction. It is not mixed into the default
x = +/-3 tables.

The disk-sampling failures occurred after valid crossings, while another run
deleted directories during `du`. The runner now uses `du`'s printed total even
when disappearing entries cause exit 1. Those valid measurements are retained
separately, not silently discarded or mixed into a successful 1,000-run table.

The first stage-3 compile had a local reader variable shadowing its reply record.
It was fixed before that configuration ran. Patch verification initially ran
`git apply` below the parent repository and silently skipped paths outside its
prefix. The helper now initializes an isolated scratch repository and compares
every resulting source file byte for byte. Its first exact comparison caught
the skipped application, so the faulty patch was not accepted.

Binary installation now copies to a new inode and renames it into place, so a
build never truncates an executable mapped by an older run. The precise OS cause
of the preflight SIGKILL was not established.

## Reproduce and verify

Base is Meridian `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`, followed by
`e2-meridian-client.patch`, `meridian-01-wall-trigger.patch`, then
`meridian-02-fast-crossing.patch`. Protocol/world dependencies remain pinned to
Horizon `0d21d5520ec26c49b774bf1533e53ea8af81278d`. Engines are the retained E2
binaries at Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`.

From the patched checkout, one command rebuilds the local macOS proxy/client,
runs four sequential 1,000-crossing configurations with edge/death probes,
checks live block sequences in stages 3 and 4, measures clocks, and cleans builds:

```sh
python3 -B scripts/e8-fast-crossing --suite --build
```

The whole suite entry point was not rerun after adding it. Its individual run,
build, probe, clock and cleanup paths were exercised separately. It does not
download, prepare, update or modify game files. Prepared 26.3 resources are read
only from:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

All engines, proxies and sidecars use unique loopback ports and fresh runtime
folders below `$REPO/.local/runs/`, with isolated `HORIZON_HOME` and XDG paths.
Every run stops its processes and deletes its worlds and logs. Only small JSON
measurements remain. No launcher account or launcher management command is used.

Builds use `CARGO_INCREMENTAL=0`, profile `fast` inheriting release, LTO off and
16 codegen units. There is one Meridian target directory. Binaries are retained
in `.local/bin/meridian-e8/`. This project has no Boat home and these binaries
must run locally on macOS.
The machine is an Apple M5 Pro with 18 logical cores and 48 GiB RAM.

Other commands:

```sh
python3 -B scripts/e8-fast-crossing --build --stage 1 --crossings 1000 --death --edge-cases --name stage1
python3 -B scripts/e8-fast-crossing --stage 2 --crossings 1000 --edge-cases --name stage2
python3 -B scripts/e8-fast-crossing --stage 3 --crossings 1000 --edge-cases --name stage3
python3 -B scripts/e8-fast-crossing --build --stage 4 --crossings 1000 --death --edge-cases --protocol-probe --name stage4-final
python3 -B scripts/e8-fast-crossing --clock --name clock
python3 -B scripts/e8-fast-crossing --verify docs/experiments/data/E8/stage4-final.json
python3 -B scripts/e8-analyse
python3 -B scripts/e8-runner-test
python3 -B scripts/e8-patch
python3 -B scripts/e8-check
scripts/clean-builds --all
```

Changed Rust files pass formatting. Scoped Clippy with warnings denied passes.
The scoped suite passes 202 config, 33 ghost, 85 handoff and 559 proxy tests.
The unchanged listener-burst test is explicitly skipped, as in E4. Workspace
formatting fails only in E2's existing `e2-client.rs`, left unchanged.
`e8-check` records that failure and returns exit 1, rather than calling it a
passing aggregate gate. The ten offline runner tests reject missing crossings,
bad route/order, checksums, timing, corrections, velocity, percentiles and
disconnects. The saved E8 patch passes application checks and exact source
comparison. Full upstream workspace and parent goliath-rs gates are not claimed.

## Evidence, disk and limits

`data/E8/` contains all runs, pilots and failed samples, binary and runner hashes,
request timestamps, clock queries, patch verification and scoped gate outcomes.
`analysis.json` is derived from those records, with per-crossing phase intervals.
Analysis does not rewrite raw trials. Results redact home paths as `$HOME` and
repository paths as `$REPO`.

The observed shared project peak was 6,429 MiB, including 6,272 MiB under
`.local/`. Meridian's single target reached an observed 505 MiB. The largest
E8 runtime folder was the 509 MiB clock run; the final crossing run used 21 MiB.
All E8 runtime folders were deleted. Retained E8 evidence is about 23 MiB.
These are sampled high-water values, not continuous peaks, and include other
agents' work in the shared project.

`scripts/clean-builds --all` returned exit 0. It deleted idle Horizon-tiles,
Horizon and Meridian targets of 1,930, 1,716 and 504 MiB, freeing 4,150 MiB and
reporting 1,720 MiB for the project immediately afterward. Other agents may
recreate their own targets; this is not a promise about their later disk use.
The final cleanup after the bookkeeping fix also returned exit 0. It freed
another 1,987 MiB from idle root, Horizon-tiles and Meridian targets, and skipped
the active 2,783 MiB Horizon target. That pass reported 4,598 MiB for the shared
project. Cumulative disposable build output removed was 6,137 MiB across the two
passes. The retained E8 binaries are outside the deleted target and remain
available.

No commit, push, GitHub action or Horizon edit was made. No Folia, Paper, GPL
source or `docs/research/05-folia.md` was read. Review is separate.
Physical momentum, a stock client, vehicles, distant positioned views, complete
terrain beyond walls, crash/partition safety, shared-store fencing and the D6
checkpoint in the input window remain unverified.

## Independent verification

The reviewer reran stage 4 on 2026-11-07 at low load, after fixing a harness bug. Two earlier reruns failed because `protocol_probe` parsed the proxy log while the proxy was writing it, and read a torn last line. The death and edge probes had passed in both. `scripts/e8-fast-crossing` now parses complete lines only.

| Metric | First run | Reproduction |
| --- | ---: | ---: |
| Crossings completed | 1,000 of 1,000 | 1,000 of 1,000 |
| Input gap P50 / P95 / P99, ms | 129.4 / 145.7 / 182.1 | 127.5 / 144.4 / 172.3 |
| Inventory equal, disconnects | 1,000, 0 | 1,000, 0 |
| Largest backward correction, blocks | 0 | 0 |
| Client arrival teleports | 0 | 0 |

These gaps don't include the D6 cut or store CAS. Raw data is in `data/E8/repro.json`.

Intermediate runs in `data/E8/` are gzipped to keep the repository small. `gunzip` them before running `scripts/e8-analyse`.
