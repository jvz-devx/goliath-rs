# E18. Proxy cost

Verdict: the reactor is not cheap enough for M4. At 975 players it uses 1.495
proxy cores and 1,992 threads, and the shared four-core diagnostic reaches only
7.23 to 7.49 tile TPS. Internal clientbound P95 is 20.30 ms with limiter pauses.
The requested M2 regressions pass; one real-physics timing window remains
unresolved. No milestone acceptance is claimed. Maintainer review and commits
are separate. Nothing from E18 has been published or sent upstream.

## Before

E12 handed over Meridian with no half-done reader changes. Its exact stack ends
at `meridian-04-m2.patch`, on `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`.
The handover source manifest is `data/E12/meridian-handover.json`.
Horizon and store authority remain outside E18 ownership.

The first profiles use the immutable E17 pilot snapshot. It is a fast build,
not release acceptance evidence. Four engines and event-loop bots run locally.
The boundary-heavy route and private admission settings are E17's unchanged
pilot. E18 imports its adapter without editing E17's files.

| Players requested | Online | Proxy cores | Peak threads | Result |
| ---: | ---: | ---: | ---: | --- |
| 50 | 50 throughout | 0.3032 | 431 | Completed, profiling perturbs timing |
| 200 | 200 initially, 85 finally | Not a steady-population number | 1,681 | Failed, 115 dropped |

Both proxy processes were sampled for five seconds after the first fixed-load
sample. `sample` observes blocked threads as well as runnable work. Blocked
socket and mutex samples are not CPU shares. Raw sampled stacks and process
counters are retained separately in `data/E18/`.

The 200-player replay retained causal warnings before bot termination. A full
cut queue caused permanent wall refusals first. Later local control connections
failed with `EADDRNOTAVAIL`, including terrain refresh, sidecars and store CAS.
The terrain-manager error path closed its real client on that failure. This
reproduces port exhaustion without packet taps. It does not prove that every
drop had that cause, or that no other resource limit exists.

The 200-player sample is dominated by socket calls and blocking waits. Its
collapsed top-of-stack counts include 14,142 send calls, 547 socket calls,
446 opens, 261 thread creations and 119 connects. Inflate has 28 samples at its
named fast helper. These counts guide ordering, not a time-accounting percentage.

## Changes under test

The first change pools authenticated Horizon control connections. Pool locks
cover only checkout and return, never network IO. Each endpoint retains at most
eight idle connections. Endpoint identities include the authentication token.
Failed IO discards the socket and returns the failure. It never replays an
uncertain command. Idle endpoint entries are bounded and can be evicted.

A definite `cut queue is full` refusal before CAS now defers the wall crossing
with a one-second backoff. Unknown CAS outcomes and activation errors keep the
existing fail-closed authority path.

## Connection-only remeasurement

These rows use a release proxy with the immutable fast tile engines. They are
unwindowed development results, not M4 acceptance. The reader model is unchanged.

| Players | Online throughout | Proxy cores | Peak threads | Peak RSS MiB | Unix syscalls/s | Context switches/s |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 50 | Yes | 0.2834 | 429 | 85.98 | 32,778 | 11,768 |
| 200 | Yes | 4.5834 | 1,694 | 439.92 | 272,611 | 63,085 |

At 200 players, user CPU is 0.5234 cores and system CPU is 4.0600 cores.
System CPU is 88.58% of the process total. The connection fix prevents the
observed dropout but does not reduce steady CPU to the working target.

### Runnable CPU profile

The release proxy completed another 45-second, 200-player run with every player
online. It includes a five-second `sample` capture and an eight-second Instruments
Time Profiler capture. The latter contains 39,236 Running samples across an
8.655-second sampled range. Their weights sum to 39.236 CPU seconds.

The percentages below divide by all Running sample weights. They do not count
blocked-thread occupancy as CPU time. Classification uses the exported stack
and leaf symbol. Allocation and copies are separate from their callers, except
when inside compression. Missing unwinds stay in the denominator.

| Work | CPU sample share |
| --- | ---: |
| Socket read/write syscalls | 85.02% |
| Allocation and byte copies | 3.95% |
| Locks, parking and scheduler calls | 3.28% |
| Ghost view state | 1.91% |
| Packet parse and rewrite | 1.51% |
| Compression | 0.89% |
| Decompression | 0.38% |
| Terrain management and composition | 0.70% |
| Protocol framing and connection buffers | 0.66% |
| Thread lifecycle and mappings | 0.57% |
| Connection and control syscalls | 0.39% |
| Control/store user code | 0.12% |
| Other user code | 0.56% |
| Missing backtrace | 0.06% |

`sendto` alone accounts for 66.52% of the total. `recvfrom` accounts for 17.28%.
The main backend-lane threads, including active and prepared lanes, account for
72.65%. Terrain readers account for 15.78%, wall workers 4.55%, client-input
threads 4.21% and terrain managers 2.67%. Other threads account for 0.13%.
Read/write syscalls account for 94.24% of backend-lane samples.

Native thread snapshots were also collected around `sample`. They omit threads
that exited between the snapshots and cover only 22.80 of 48.19 process CPU
seconds in that perturbed interval. They are retained as incomplete evidence,
not substituted for the Instruments denominator.

The working proxy budget at 200 players is 0.1231 cores if 0.6 cores must cover
975. Today's measured 4.5834 needs a factor of 37.24. Removing all compression
cannot supply that. Even ideal removal of every terrain reader and manager
sample would leave about 3.74 cores at 200 under a fixed-work Amdahl illustration.
That is not a prediction of a new reader design.

macOS loopback syscall costs may differ from Linux. No Linux comparison has
been measured. This pass line is measured on the Mac and remains a failure here.

## Write coalescing

The reviewer selected incremental write coalescing before a reader change or an
event-loop redesign. Packets already buffered by a 64 KiB read are drained before
the next blocking read. Client and backend writers queue complete framed bytes
in order, preserving compression and cipher state. A completed drain flushes
without a timer. Its queued buffer never exceeds 64 KiB. One larger legal packet
uses the existing bounded protocol path. A failed partial write latches the
writer closed and discards the uncertain buffer without replay.

Overlapping producers flush on each completed drain, so an unrelated busy lane
cannot indefinitely hold a control reply. Handover deltas and queued-input replay
use the same bounded coalescing. `E18_WRITES_DIRECT=1` retains the direct-write
ablation. No event-loop redesign has started.

The initial eager coalescing pilot kept all 200 players online and used 1.6808
proxy cores with 1,693 peak threads, 193,249 Unix syscalls/s and 63,338 context
switches/s. The 50-player row used 0.2343 cores. Both ran above host load 6.
They are diagnostic results only, not accepted CPU or latency measurements.

## Lazy readers

Owned terrain now comes from the actual active player lane. A foreign reader
opens within view distance plus two chunks of its tile. It stays until four
extra chunks away, giving two chunks of hysteresis. There is at most one live
reader per configured tile and player. An own-tile reader is prewarmed only near
a foreign tile. It already exists when a crossing turns the old source into a
foreign view. Activation readiness additionally requires all still-visible
source columns to have arrived on that reader before releasing the real source
lane. A border-heavy route still needs every neighbour and is not an interior
reader-cost test.

Retirement rechecks the latest pose under session locks. It marks the reader
intentionally closed before forgetting invisible foreign columns. The reader
cannot repopulate the retired view afterward, and its deliberate EOF cannot
disconnect the real player. Each reopening uses a fresh synthetic UUID. A known
expected-player admission refusal backs off without closing the real client.
Registry mismatch and an unexpected lost live reader retain fail-closed handling.

The release candidate's 512-block lifecycle fixture passed zero readers in the
interior, four preloaded at a corner, the same four UUIDs in the hysteresis band,
and zero after retirement. Two 32-block/s kinematic flight approaches through
the corner had zero missing frames, missing columns or visible reloads. This
does not establish real physics momentum. The first ground-diagonal fixture
hit E10's deliberate gold world marker at 8,8 and was corrected repeatedly.
That failed run remains. The replacement flies above the block, with ordinary
creative flight and no relaxed server validation.

The earlier 14-crossing terrain pilot passed the E12 packet auditor, live
foreign block update, refused foreign break and foreign entity exclusion with
an owner positive control. Final 100-crossing, 1,000-crossing, item, owned-border,
sprint and teleport regressions are still pending.

## Latency instruments and quiet contract

The full Python tap at 200 players reported a 411.926 ms path P95 while proxy
CPU dropped to 0.557 cores. This tap changed traffic delivery and is intrusive
evidence, not a proxy-added latency result. No sample or tail was trimmed.

The replacement preselects eight witnesses from seed 18, two from each early
route phase. It validates their physical spread using the first frontend
movement packet from each witness. Held and real sessions can appear on two
tiles at once, so server-status de-duplication is not used for that geometry.
Every other player and reader still traverses the identical extra TCP hop and
forwarding queues, but its packets are opaque after Hello. Acceptance requires
all four tiles represented, at least two witnesses within 32 blocks of a wall,
and a witness crossing activation within the measurement window. Observer and
driver CPU are separate from proxy CPU.

An optional bounded internal histogram records every forwarded player's decoded
read-burst timestamp through completed socket write. Later frames include time
spent behind earlier frames in that read buffer. The first frame's decompression
before `recv` returns is outside this clock, so it is not an end-to-end wire
latency measurement. Frozen inputs keep their original stamps. Inputs consumed
inside activation have a separate read-to-activation-ack histogram because they
do not have a game-socket write. Generated writes without a frame origin are
counted explicitly as unstamped rather than assigned a fabricated zero.
Ten-microsecond bins retain every sample; values at or above 100 ms stay in an
overflow bucket with the actual maximum retained.

The reviewer refined the original load rule after the first windows. A cell starts
at load at most 4. Whole-host process CPU is sampled every second; no process
outside the fixture may average over 0.5 core during the cell. Fixture-owned
runnable threads can raise load above 6 without rejecting the cell. Earlier rows
without this outside-process sampling are not retroactively accepted. Final
proxy cells use the existing E11 userspace allocation method:
four aggregate cores for engines, proxy, sidecars and store, and a separate
one-core bot cap. TPS and bot lateness must expose throttled or stalled work.
The observer is outside the server budget and reported separately. Controller
SIGSTOP/SIGCONT cycles, number of process stop signals, wall pause intervals and
summed per-process pause seconds accompany each latency window. No pause time
is subtracted from P95.

The first quiet request was released after 252.8 seconds. Its two uncapped rows
are not accepted. The second row also exposed a runner bug: the wrapper queried
game time again during final save after the tiles had stopped. That query now
runs only for live measurement samples. The next request was interrupted when
the reviewer requested process attribution before another 200-player cell. The
already-running 200 cell finished before its interrupt could be delivered; it
is retained, not claimed as a new process-attributed measurement. A later
50-only process diagnostic passed the refined noise rule. All requests are
bounded and released after their cells. E12 and E14 are finished. Once the
reviewer's E13 campaign finished too, the reviewer waived its acknowledgement;
later diagnostics require only E17's acknowledgement and the start-load gate.

## Tile-cost diagnosis

The process-attributed 50-player release cell used 3.643 cores in its four tiles
and 0.345 in Meridian after a 30-second warm-up. Store and sidecars used 0.013.
The server sum was 4.000 cores under the four-core limiter, with all tiles near
20 TPS. The largest outside-process average was 0.090 core. Its latency window
contained 2,005 limiter stop cycles, 20.310 seconds of controller pause intervals
and 198.421 summed per-process pause seconds. These pauses are not proxy CPU or
a correction to the measured latency.

The reviewer requested uncapped controls before further proxy work. All use the
same FINAL release engine and configuration where applicable, a 30-second
warm-up and 30-second window. Plain mode disables cluster, ownership, cut,
lease and digest runtime opt-ins. No implementation changed for these cells.

| Control | Real players | Reader sessions | Tracked chunks | Engine CPU cores |
| --- | ---: | ---: | ---: | ---: |
| Plain FINAL, empty | 0 | 0 | 0 | 0.0053 |
| Plain FINAL, 12 route bots | 12 | 0 | 144 | 3.5656 |
| Empty cluster tile a | 0 | 0 | 9 | 2.6145 |
| Empty cluster tile b | 0 | 0 | 9 | 2.6684 |
| Empty cluster tile c | 0 | 0 | 9 | 2.7514 |
| Empty cluster tile d | 0 | 0 | 9 | 2.6551 |
| 50-player cluster tile a | 12 | 50 | 49 | 2.5456 |
| 50-player cluster tile b | 11 | 50 | 49 | 2.6608 |
| 50-player cluster tile c | 13 | 50 | 49 | 2.6738 |
| 50-player cluster tile d | 14 | 50 | 49 | 2.6571 |

The 50-player rows also had two, two, three and two held real sessions during
handover. They are not counted as additional bots. Every outside process stayed
below 0.228 core in these controls. Start loads before fixture construction were
below 4. The fixture itself raised later load, which the refined rule permits.
The 10-second sample selected tile a using the highest preceding five-second
CPU delta. Its main thread was actively pumping chunks inside its tick-wait
function, not merely sleeping. Worker notifications dominated that path.

### Matched binary comparison

The retained unpatched-looking E2 baseline was a `fast` build, so E18 exported
clean upstream `fc2868b2b26d56ec76e17414909fe58a3b87bb92` into a disposable tree
and built native release with ThinLTO, one codegen unit and no incremental
compilation. No patch was applied. The build passed in 286.04 seconds and its
source and target were deleted. Commands and hashes are in
`data/E18/upstream-fc-release-build.json`.

| Engine | SHA-256 | 12-player CPU cores | Chunks |
| --- | --- | ---: | ---: |
| Exact E17 W1 | `8dbfabbecf16be8a19fd3297f29b976de4a60c610d8b848afdd0fdd3f53fc928` | 3.5653 | 144 |
| Clean upstream release | `5548ae803a7826be310462147fdcb4e9c3fb9c69627758c2978473a0351c45da` | 3.5345 | 144 |
| FINAL release | `7b21187dcc45e474af46821d82fa5c77deaaa78cb69632c8af8e4f9991ae2978` | 3.5564 | 144 |

W1 with zero players but 144 chunks held by `/forceload` still used 3.1938 cores.
All four cells ran at about 20 TPS. Thread CPU uses `proc_pidinfo` nanosecond
thread counters, with coverage within 0.001% of the independent process CPU
counter. At 12 players, the main thread used about 0.90 core and five workers
about 0.52 to 0.53 each. Top-eight named-thread data is retained for each cell.

CPU cores in the top-eight thread sets, aligned by name:

| Thread | W1 | Upstream | FINAL |
| --- | ---: | ---: | ---: |
| `main` | 0.9063 | 0.9037 | 0.9059 |
| `horizon-worker-0` | 0.5324 | 0.5250 | 0.5261 |
| `horizon-worker-1` | 0.5280 | 0.5232 | 0.5307 |
| `horizon-worker-2` | 0.5285 | 0.5248 | 0.5263 |
| `horizon-worker-3` | 0.5304 | 0.5248 | 0.5279 |
| `horizon-worker-4` | 0.5291 | 0.5222 | 0.5290 |
| `horizon-net-0` | 0.00164 | 0.00170 | 0.00165 |
| `horizon-net-1` | 0.00155 | 0.00159 | 0.00156 |

The reviewer then requested the original 325-player W1 workload, not the narrow
12-player route. Reusing `capacity-bench` with only thread snapshots added kept
seed 12345, view distance 3, the seeded 128-block patrol area, 64 invulnerable
AI-enabled pigs and 30 seconds of warm-up. With the server uncapped it passed:
1.4960 engine cores, 19.990 TPS and tick P99 46.856 ms. All 325 players, 64 mobs
and 324 chunks remained. Bot CPU was 0.114 core with no limiter stops or late
rounds. The main thread used 0.989 core; the five workers fell to 0.223 combined.
This reproduces the published passing 325 rows at about 1.47 cores. The older
roughly 1.29-core rows belonged to the failing 350-player step.

The inverse CPU trend is consistent with busy simulation ticks leaving less
idle pacing time to feed the retry loop. It is not a changed process-CPU unit
or a FINAL-only binary difference. The 325 and 12 workloads also differ in
route footprint and mobs; no population-only scaling claim is made.

### Upstream loop and local patch 09

At upstream `fc2868b2b`, `game/commands/pregen.rs::Game::pump_chunks` loops while
`Streaming::busy`, waits for a completion and calls `update_world` again before
the next tick. `Streaming::light_region` queues `Job::LightView` for a decorated
proto neighbour whose view is absent. The simple `FlatTerrain` path inherits
`ChunkGenerator::light_view` returning `None`. `receive_light_view` restores
the proto but caches only `Some`, so the same region can be eligible again
without new terrain work. The flat configuration used here selects that simple
generator because structures, features and lakes are disabled.

`Streaming::submit` really pushes a job before notifying. `Inner::notify` checks
sleepers, not the job queue; an awake worker can consume the job between that
enqueue and notification. That ordinary race alone is not evidence of an
empty-queue spin. The actionable condition is the unproductive light-view retry.

The loop and default are present in clean upstream. Our E6
`horizon-01-tile-cut.patch` additionally installs a one-millisecond control-pacing
branch in `game/levels.rs`, gated by `HORIZON_E3_CUT_DIR`; ordinary mode keeps
upstream's original pump. No hunk changing that pacing or notification code is
included in patch 09.

`horizon-09-flat-light.patch` supplies a cloned read-only view of FlatTerrain's
already-materialized `Chunk` proto. Existing normal lighting can then finish.
It does not mark an unlit chunk final, change ownership or alter condvar rules.
The production change is three lines. Resource-free 3-by-3/COW and real streaming
quiescence tests accompany it. The first test invocation exposed Horizon's
explicit test registration and split library package, before compilation; that
no-target error is retained and not counted as a red test. The registered
streaming test failed before the fix with its ten-second retry timeout. Both
W1-equivalent and FINAL native release stacks then passed the unit test,
streaming test, strict Clippy and release build. Disposable source and targets
were removed. `flat09-both-stacks-proof.json` records source byte equality on
both stacks. The canonical patch hash is
`10ae7266c7ffb287fd73239e8e8d43031b87fa1382a838c65d71edfff11d408e`.

This is an upstream issue patched locally. Nothing was sent upstream. Both
stacks must receive the exact same patch. E17 must repeat the patched plain
staircase before W2; the old 325 baseline and 975 target are not fixed targets.

After 09, the reviewer's working service budget is about 0.8 to 1 core at the old
975-player illustration, roughly 0.001 core per player. This replaces the initial
0.6-core proxy estimate as a rough planning line, not an accepted capacity claim.
The next measurement is the production proxy slope. If it remains above that
line, an event-loop proposal must come
back with measurements before implementation. Lazy readers alone are not claimed
to close the gap.

### Matched post-09 controls

These five uncapped release cells each used 30 seconds of warm-up and a
30-second measurement. They passed the start-load and outside-process rules.
The largest outside-process average was 0.064 core. No limiter ran.
`flat09-five-cells-1.json` retains the per-process counters and populations.

| Cell | Before 09 cores | After 09 cores |
| --- | ---: | ---: |
| Plain A12, engine | 3.5653 | 0.0386 |
| Plain zero players, 144 loaded chunks, engine | 3.1938 | 0.0067 |
| FINAL A12, engine | 3.5564 | 0.0383 |
| True B0, complete fenced cluster | 10.6928 | 0.0892 |
| C50, complete fenced cluster | 10.8400 | 0.7175 |

B0's four tiles used 0.0206, 0.0216, 0.0203 and 0.0206 core. C50's tiles used
0.0991, 0.1021, 0.1061 and 0.1025 core; its proxy used 0.2977. Each C50 tile
still had 50 eager terrain readers and 49 chunks. This C50 deliberately kept
direct writes and eager readers to match the pre-09 diagnostic. Its proxy cost
is not the production coalesced/lazy result.

### D9 production window

The pass layout follows D9, not the origin-junction stress route. Four
128-by-128 patrol squares sit at tile centres with coordinates 1,024 or -1,024
on a 2,048-block grid. Ninety percent of players are split over those squares;
the other ten percent continuously cross walls and a corner. Each tile receives
64 invulnerable AI-enabled pigs. The matched plain denominator uses the same
four squares, crossing route and 256 pigs on one process. W1's original
one-square, 64-pig workload is a reference only.

The benchmark-only tap assigns the initial backend through existing forced-host
configuration, rewriting only the initial handshake host. Interior actors are
then moved to their seeded start within their already-owned tile, as in the
plain benchmark. Crossing actors use the fenced path. Every actor traverses
the same front and backend tap hops. Six interior and two crossing witnesses
are selected with seed 18 before startup, including an interior witness on
each tile. Unmatched frontend packets stay in the conservative P95 denominator.

`scripts/e18-production-slope` runs the approved 50/200 window with the shared
four-core server cap and one-core bot cap. It records per-process cold-join,
warm-up and steady CPU, one-second outside-process samples, reader and real
player counts, pig population, proxy syscall/context-switch counters, internal
histograms, witness latency and controller STOP intervals. This is measurement
code only. The event-loop delta is developed separately from this candidate.

The approved window completed in 152 seconds. Both 30-second cells began below
load 4 and passed the outside-process rule for both steady and whole-cell
sampling. The highest outside-process average was 0.0573 core. All bots stayed
online with no late rounds. Each tile retained 64 pigs. The request and ack were
removed immediately after cleanup; E17 then received the load slot for its new
plain-09 baseline.

| Players | Proxy cores | Cores/player | Peak threads | Peak RSS MiB | Unix syscalls/s | Context switches/s |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 50 | 0.08299 | 0.001660 | 236 | 81.27 | 9,244 | 5,643 |
| 200 | 0.26498 | 0.001325 | 904 | 398.70 | 37,204 | 22,925 |

| Process | 50 cores | 200 cores |
| --- | ---: | ---: |
| Tile a | 0.09753 | 0.15115 |
| Tile b | 0.09853 | 0.15148 |
| Tile c | 0.09387 | 0.14615 |
| Tile d | 0.09853 | 0.15747 |
| Proxy, ps counter | 0.08288 | 0.26501 |
| Store | 0.00266 | 0.00333 |
| Sidecar a | 0.00033 | 0.00067 |
| Sidecar b | 0.00033 | 0.00033 |
| Sidecar c | 0.00033 | 0.00033 |
| Sidecar d | 0.00033 | 0.00033 |
| Server total | 0.47533 | 0.87626 |
| Observer plus measurement driver, excluded from server total | 0.19739 | 0.44945 |
| Five bot drivers combined, separate cap | 0.02630 | 0.08290 |

Tile main-thread CPU was not captured in this window. These process averages
are upper bounds on it, not measured thread values. Native-ID thread snapshots
are now included in the D9 runner for W2. No value is reconstructed from process
CPU. `scripts/e18_process.py`, imported by E17, remains unchanged.

| Cell | Placed players a/b/c/d at start | Held a/b/c/d at start | Readers per tile | Chunks a/b/c/d |
| --- | --- | --- | ---: | --- |
| 50 | 14/12/12/12 | 0/0/0/0 | 5 | 335/331/331/331 |
| 200 | 46/49/51/54 | 0/3/0/1 | 20 | 374/376/384/384 |

Held arrivals are separate from real placed players. Transient held sessions
can make the session sum exceed the client population. End-of-window placements
and per-player tracked chunks are retained in the raw result.

| Players | Witness path P95 ms | Matched/front | Internal clientbound P95 ms | Internal serverbound P95 ms | Limiter pauses in latency window |
| ---: | ---: | --- | ---: | ---: | --- |
| 50 | 0.460 | 4,818/4,818 | 0.030 | 0.010 | None, both caps |
| 200 | 1.779 | 4,808/4,812 | 0.030 | 0.010 | None, both caps |

Witness P95 puts unmatched frontend packets at infinity rather than dropping
them. Both crossing witnesses crossed inside each measurement window, and
corner transfers were observed. The observer forwards all other actors on the
same path. Its CPU is reported separately above. Internal histograms cover
486,624/2,846,113 clientbound writes and 90,999/363,856 serverbound writes.
Generated unstamped clientbound writes numbered 1,113/9,186 and are explicitly
outside that histogram. Internal timing begins after the first decoded frame
of a read burst, so it does not cover that frame's decompression. Witness timing
includes the observer and TCP queues and is not a subtraction from a direct path.

Neither cap issued a STOP during either latency window: zero cycles, zero
process-stop signals, zero stopped wall seconds and zero summed process-pause
seconds. This is recorded next to the P95 values, not subtracted from them.
All raw tick buffers had zero drops; their largest tick was 10.631 ms at 50
and 10.545 ms at 200. No 975-player cell was run.

Raw evidence is `production-D9-50-200-1.json`. Its eligibility boolean initially
failed a reporting-only UUID join because status uses hyphenated UUIDs and
phase logs use compact hexadecimal. The raw file is unchanged.
`production-D9-50-200-1-summary.json` normalizes that join, records its raw hash,
and confirms both witness checks. Eleven harness tests passed. The guard for
future 200-player windows is restored and requires explicit window approval.

### Event-loop proposal

The measured proxy slope between 50 and 200 is 0.0012133 core per added player,
with a 0.0223-core fitted intercept. A two-point extrapolation gives 1.205 proxy
cores and roughly 4,355 threads at 975. The 0.6-core line would be crossed at
about 476 players in that model. Neither value is a measured high-load result.

An affine extrapolation of tile CPU is rejected. Within one fixed patrol area,
players track one another and work can grow faster than player count. The
reviewer's illustrative quadratic estimate gives about 0.703 core per tile at
244 players, or 2.813 across four. Adding the current proxy gives about 4.018
cores before store and sidecars. There is no ten-percent headroom. The new
plain-09 staircase and W2 must replace this illustration with measurements.

The next local Meridian delta will replace hot per-connection blocking IO with
a small fixed reactor pool. Each connection keeps its framing and cipher state,
FIFO output order and bounded pending-byte count. Reads drain complete frames;
writes coalesce at each ready-work drain, with no batching timer. A slow socket
must not block the pool or grow an unbounded queue. Cut, store and backend-login
operations run outside the reactor on bounded workers. Definite pre-CAS
refusals defer with backoff; an uncertain CAS never resumes the source.
Unchanged-frame reuse is allowed only where the existing entity, teleport and
terrain rewrite rules prove it safe. Authority and leases do not change.

The planning expectation is an incremental proxy slope of 0.0007 to 0.0009 core
per player, about 0.705 to 0.900 core at 975 with the current fitted intercept.
This needs roughly 35 to 45 percent less system CPU at 200, where system work
is 72.55 percent of proxy CPU. It is not an achieved saving. Under the reviewer's
tile illustration the low end leaves about twelve percent headroom, while the
high end leaves only seven percent before store and sidecars. The target is
therefore near 0.0007, not merely below 0.001. W2 decides whether it is enough.

Implementation is staged behind an explicit candidate selection. FIFO,
fragmented-frame, bounded-backpressure, partial-write and switch/cancellation
tests precede release and M2 gates. Native per-thread snapshots are required
in W2. No new build or load overlaps E17's quiet baseline window. The reviewer
authorized coding after this proposal without a further approval round.

The first isolated transport gate passed three native release tests in 50.15
seconds, outside E17's quiet window. `reactor-unit-1.json` records its exact
source hashes and command. Follow-up owner-drop hardening and extraction of
the existing client/backend packet-transition kernels are not covered by that
first gate. At that stage the transport was not yet wired into the session.
The later integrated candidate is described below and remains separate from
the frozen patch-05 candidate.

The 975-player witness fixture also needs an intrusion check. The single Python
observer plus driver used 0.449 core at 200. It may saturate before 975 even
though only eight actors are decoded. A tap-free W2 capacity cell with internal
histograms, or a partitioned observer with identical actor paths, is needed to
avoid treating observer throttling as a proxy saving. The old fully decoded
411 ms result remains explicitly invalid for that reason.

## Frozen patch-05 diagnostic at 975

E17 completed the matched one-process D9 baseline with 256 pigs. At 325 players,
two runs passed at 19.9986/19.9975 TPS and tick P99 46.915/47.168 ms. Engine
CPU was 1.061/1.053 cores and its named main thread 0.828/0.823. At 350,
both runs failed P99, 56.363/57.456 ms, with main CPU 0.978/0.980 and no
limiter stops. This re-establishes 975 as the tested-step 3x line; 1,050
remains the conservative bracket line. It is not carried from W1.

The first 975 attempt failed during joining, before warm-up or measurement.
The fixture left interior actors at the corner until everyone joined, briefly
creating junction readers for almost the entire population. Eleven terrain
managers logged `EADDRNOTAVAIL`; that error closes the real session in the
current product. Five shims reported no errors and queue peaks below 55 KiB.
No steady CPU or capacity value is taken from that attempt.

The cleanup printed a `tile-cut` reply of `game exited`. That is the generic
disconnected-reply-channel fallback in Horizon `control.rs`, not an OS process
exit status. I initially described it too strongly as a tile exiting during
join. Native engine stderr and return codes were not retained before cleanup,
so panic, a cut-worker failure and the exact game lifetime cannot be resolved
from that run. No matching system crash report was found. This does not prove
that no panic occurred. The lease-expiry branch keeps diagnostics alive and
would normally return an explicit lease refusal. No lease-expiry or M3 claim
is made from the cleanup string.

The corrected fixture boots each tile at its interior centre. Five cohorts
join at a 0.25-second cadence. Each crossing actor is placed at `[12,-58,12]`
within its original owner a as it joins, then follows the unchanged wall route.
The steady layout, patrol seeds and pig count are unchanged. Failure capture
now retains engine warnings, panic/error text and child return codes before
any cleanup. Socket pressure is sampled throughout startup and steady state.

`production-D9-capacity-diagnostic-2.json` completed 30 seconds of warm-up and a
60-second measurement on frozen patch 05 and FINAL-09. Every bot stayed online,
every tile retained 64 pigs and 373 chunks, and each tile had 97 terrain readers.
No terrain connection failed. This is a diagnostic, not M4 acceptance.

| Tile | Placed at start | Held at start | Cores | Native main cores | Game-time TPS | Raw tick P99 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 241 | 3 | 0.53452 | 0.30702 | 5.6523 | 223.262 |
| b | 250 | 4 | 0.53751 | 0.30589 | 5.3009 | 228.611 |
| c | 235 | 1 | 0.53418 | 0.31094 | 5.7077 | 228.452 |
| d | 252 | 4 | 0.53900 | 0.30675 | 5.2921 | 250.549 |

Sequential status queries can straddle handovers, so their placed sum is not
a simultaneous client census. The five bot drivers consistently reported 975
online. All raw tick buffers had zero drops. Native thread IDs use flavors
28/15; the main thread is selected by its name, not a pthread address.

The proxy used 1.82921 cores, 0.56137 user and 1.26784 system, with 4,350 peak
threads and 1,084 MiB peak RSS. It performed 131,446 Unix syscalls and 108,890
context switches per second. The store used 0.01328 core and each sidecar
about 0.00149. Server CPU was pinned at 3.99252 cores. These capped costs are
not an uncapped demand estimate. Proxy cost per player was 0.001876, above
the earlier 50/200 extrapolation. It is above the original 0.6-core line at
975; the exact population at that crossing has not been measured.

The server limiter issued 2,475 stop cycles and 24,750 process-stop signals.
Controller STOP-to-CONT wall time was 42.5687 seconds, with 386.399 summed
process-pause seconds. Internal clientbound P95 was 0.470 ms and P99 34.400 ms;
serverbound P95 was 0.020 ms. Those figures include limiter pauses that occur
after their read timestamp, but exclude kernel queueing before it. Activation
ack P95 overflowed the 100 ms range: 6,752 of 7,026 samples overflowed, with a
627.759 ms maximum. No external witness P95 was measured in this diagnostic.
Low ordinary-forwarding P95 does not excuse the failed TPS.

Bots used 0.31965 core total and reported no late rounds, failures or falling
behind. Their limiter had two brief stop cycles, 11.495 ms of stopped wall
time and 57.145 ms summed process time. The runner's conservative any-bot-STOP
gate flags this, despite the substantial average bot headroom. The server
TPS and tick P99 independently fail by wide margins. The conditional 1,050
cell was not started.

The five test-only hostname shims used 0.50031 core combined. No shim would be
needed for a real client's normal virtual-host handshake. They are reported
and excluded from server CPU, as requested. Their peak queues stayed below
32 KiB and none logged an error. Measurement/controller CPU was 0.37677 core,
also separate. Start load was 1.058; the largest outside process averaged
0.06775 core over the steady window, so the host-noise rule passed.

Peak proxy ownership was 2,473 logical open TCP sockets, including the listener,
1,497 outbound connections and 7,639 TCP file descriptors including clones.
Loopback TIME_WAIT peaked at 3,906 rows. At most 6,405 distinct loopback ports
in the 49,152 to 65,535 range were observed. The steady-window unique-tuple
creation rates were 55.38/s for observed proxy/exclusive destinations and
82.24/s for all observed fixture destinations. These are observations, not
exact connect-syscall counts. PID sampling misses some short-lived shared
store/control connections. `nettop` missed all 20 short-lived connections in
a calibration; `netstat` TCP counters were zero on this Mac, and DTrace
required unavailable privileges. No host network settings were changed.

The corrected join stayed far from the earlier tuple-exhaustion condition.
The old connect-error session-close path remains a reported product defect,
not a failure hidden by dropping clients from an average. The reviewer chose
reactor wiring because the proxy takes 45.8 percent of the capped server CPU,
69.3 percent of its own CPU is system time, and the limiter repeatedly pauses
all tiles. Patch-06 results must be measured, not projected from this run.

## Reactor candidate

`reactor-1` is a separate native release snapshot with proxy SHA-256
`29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b`.
Its engine, bots, store, sidecar and packet-client hashes are unchanged from
the FINAL-09 prerequisite bundle and frozen patch-05 candidate.

The executable owns four IO drivers and four bounded command-hook workers.
A session's client, active/held lanes and readers share one driver affinity.
Local forwarding posts ready work without a wake syscall; cross-thread control
requests use a waker. Per-socket output is FIFO with an 8 MiB byte cap and an
8,192-chunk cap. Partial writes keep offsets; write-completion callbacks, not
queue insertion, finish timing samples. Admission is bounded per driver.
Client packets waiting on a routing gate or command hook have a separate
bounded queue. Saturated command workers refuse a command before execution
and leave the session alive. Store, cut and lease authority are unchanged.

Per-player supervisor and terrain-management threads remain. This candidate
does not claim a constant total thread count. It replaces the hot per-socket
IO threads first. The existing packet-transition kernels serve both transports,
so framing/scheduling changes do not duplicate entity, teleport or phase rules.

Release formatting, strict Clippy and tests passed in `reactor-gates-5.json`:
202 config, 33 ghost, 96 handoff, 104 net, 562 proxy, two signal and 99 sidecar
tests. One internet integration test was ignored and the known listener-burst
test skipped by its exact name. New TCP tests cover compression/encryption,
graceful disconnect ordering, a blocked command not stalling another client
on one IO driver, and buffered movement resuming when a routing gate opens.
A deterministic regression also proves that writes queued before registration
survive an early dirty notification. The earlier strict gate rejected an
oversized command enum and test placement; both were fixed without suppressions.

The real-process 14-crossing smoke and all-tile inventory/restart check passed.
The final native proofs below and exact 975 rerun use this same binary.

### Final M2 regressions

| Proof | Outcome |
| --- | --- |
| E10 crossings | 1,000/1,000, zero disconnects, zero maximum correction, 1,000 visible CAS generations |
| E10 restart | All four tiles restarted; inventory equal; one current player owner at generation 1,000 |
| E10 item fixture | Alpha 7 and Beta 11 conserved through chest, drop, pickup and all-tile restart; no loose duplicates |
| E12 wall terrain | 100 crossings; zero missing columns, missing frames or visible reloads |
| Owned borders | All 16 walls, 24 expected columns each, zero missing |
| Live state | 14 transfers; attributes, flags and abilities retained; sprint modifier +0.3 preserved without an empty intermediate list |
| No-input teleport | Two complete views loaded without client moves, then a successful crossing |
| Lazy lifecycle | Interior zero readers, four corner readers, same UUIDs through hysteresis, zero after retirement |
| Fast approach | Both 32-block/s flight corner approaches had zero missing columns, frames or reloads |
| Real physics | 112 actual crossings over walk, sprint, sprint-jump and fly, visual option off/on; zero correction failures and 28/28 sprint packet checks |

The unchanged E14 checker reports `passed`, with 111 of 112 timing windows
resolved. The visual-off flight a-to-d window at index 52 has a frame interval
over 100 ms, so its timing remains unresolved. It is retained, not trimmed or
relabelled as a measured pass. The 55 resolved visual-off windows have speed
ratio at least 0.9648; all 56 visual-on windows resolve and have ratio at least
0.9990. All wire correction budgets resolve with zero backward correction.
These are implementation regression results, not an independent milestone
acceptance. Raw physics runs and the unchanged checker output are under
`data/E18/physics/`.

The E10 input-gap P95 is 137.968 ms. That includes handover and server ticks;
it is not the ordinary per-packet added latency metric. The complete native
crossing proof took 533.08 seconds. Each fixture removed its world and logs
after retaining its result. The E14 runner and checker were imported or run
unchanged, using an explicitly labelled E18 candidate snapshot contract.

### Exact 975 reactor diagnostic

`production-D9-capacity-diagnostic-3.json` repeats the corrected D9 fixture
from `-2`: frozen FINAL-09 engine, same store/sidecar/bot bytes, 30 seconds
warm-up, 60-second measurement, shared four-server and one-bot core caps,
interior spawns, batch cadence, route seeds and 256 pigs. Only the proxy build
changes. Native proxy thread snapshots are additionally retained outside the
main CPU-counter span. The new fixed-IO startup line is recorded.

The quiet window lasted 266.85 seconds and was released after cleanup. Start
load was 1.675 and the largest outside process averaged 0.06134 core during
measurement. The host-noise rule passed. All 975 bots stayed online with no
errors, falling behind or late rounds. No reader connect failed, no game-exit
error appeared, and no shim failed. The single warning is the launcher's
expected TCP liveness probe closing before login.

| Metric at 975 | Frozen patch 05 | Reactor patch 06 |
| --- | ---: | ---: |
| Proxy cores | 1.82921 | 1.49510 |
| Proxy user cores | 0.56137 | 0.55757 |
| Proxy system cores | 1.26784 | 0.93754 |
| Cores/player | 0.001876 | 0.001533 |
| Peak threads | 4,350 | 1,992 |
| Peak threads/player | 4.462 | 2.043 |
| Incremental threads/player above idle | 4.452 | 2.026 |
| Peak RSS MiB | 1,084.19 | 951.06 |
| Peak RSS MiB/player | 1.112 | 0.975 |
| Unix syscalls/s | 131,446 | 166,159 |
| Context switches/s | 108,890 | 56,455 |
| Mean tile TPS | 5.488 | 7.410 |
| Capped server cores | 3.99252 | 3.99403 |

Observed proxy CPU fell 18.3 percent, threads 54.2 percent and context switches
48.2 percent. Syscalls rose 26.4 percent. Throughput and packet counts also
changed, so this is not a fixed-throughput efficiency estimate. Nonblocking
drains can add unsuccessful read calls, but no post-change syscall-site
profile was taken to attribute the increase.

| Tile | Placed/held at start | Readers | Pigs | Cores | Native main cores | Game-time TPS | Raw tick P99 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 258/5 | 97 | 64 | 0.62708 | 0.37369 | 7.4868 | 176.070 |
| b | 228/2 | 97 | 64 | 0.61910 | 0.37280 | 7.4720 | 171.735 |
| c | 242/4 | 97 | 64 | 0.61427 | 0.37018 | 7.2313 | 167.069 |
| d | 250/3 | 97 | 64 | 0.61976 | 0.37319 | 7.4518 | 177.701 |

All tiles retained 373 chunks and no raw tick samples were dropped. The store
used 0.01214 core; sidecars used 0.00150 to 0.00166 each. Bots used 0.45534
core total, shims 0.64137 and measurement/controller 0.32285. The latter two
remain separate from production server CPU.

| Latency and limiter | Patch 05 | Patch 06 |
| --- | ---: | ---: |
| Internal clientbound P95 ms | 0.470 | 20.300 |
| Internal serverbound P95 ms | 0.020 | 17.570 |
| Internal reader-reply P95 ms | 0.190 | 18.590 |
| Server stop cycles | 2,475 | 3,074 |
| Server process-stop signals | 24,750 | 30,740 |
| Server stopped wall seconds | 42.5687 | 35.6297 |
| Server summed process-pause seconds | 386.399 | 337.494 |
| Bot stop cycles | 2 | 84 |
| Bot stopped wall seconds | 0.01149 | 0.68895 |
| Bot summed process-pause seconds | 0.05714 | 3.42195 |

Both P95 columns include SIGSTOP exposure after their read timestamps. Neither
includes kernel queueing before those timestamps. The reactor starts timing
at the ready-read drain before buffered decoding; the legacy path starts after
the first decoded frame. The reactor also retains queued output until physical
write completion. Do not attribute the full P95 difference to one cause without
separating queue residence and process stops. No external witness P95 at 975
was measured. The internal result alone does not satisfy the 5 ms working line.

Clientbound timing contains 48,056,639 samples and excludes 336,404 generated
unstamped writes. Serverbound timing contains 2,819,931. Activation-ack P95
again overflows 100 ms: 4,148 of 4,868 samples overflow, maximum 423.958 ms.
The conservative any-bot-STOP gate flags the row even though average bot CPU
has headroom and all lateness counters remain zero. TPS and tick P99 fail
independently and decisively. The 1,050 cell was not started.

Socket pressure remained controlled: peak 2,462 logical proxy TCP sockets,
1,486 outbound connections and 7,624 TCP file descriptors including clones.
Loopback TIME_WAIT peaked at 3,752 and observed ephemeral ports at 6,207.
Observed steady unique-tuple creation rates were 49.52/s for proxy/exclusive
destinations and 75.78/s for all fixture destinations, with the same sampling
limitations as `-2`. All five shim queues stayed below 33 KiB.

Matching native thread endpoints attribute 0.94278 core to the four IO drivers,
including 0.48322 user and 0.45955 system. The 975 terrain-manager threads
take 0.45134, including 0.40303 system. The four command workers are idle.
Matched threads cover 1.39582 of the proxy's 1.49510 cores; short-lived wall
workers are not fully covered. The management polling is a substantial next
target. Even optimistically removing all measured manager CPU leaves about
1.044 cores at this already-failed throughput. That is not a capacity forecast.

The reviewer's expected slope of 0.0007 to 0.0009 core/player was not achieved.
At the observed rate, the initial 0.6-core line would need another 59.9 percent
proxy CPU reduction; the revised roughly 0.001 core/player line needs another
34.8 percent. Reaching 20 TPS changes traffic, so those percentages do not
promise sufficient capacity. A shared manager scheduler and read/write fairness
are bounded follow-up candidates, not implemented results. The current driver
processes its ready-read set before its write set; queue-residence measurement
would help separate that scheduling cost from SIGSTOP tails.

macOS loopback syscall cost may differ from Linux. The pass line is on this
Mac, so the measured failure counts. No Linux result is substituted. Per the
reviewer's final handoff direction, no further redesign or load cell was started.
Final patch-06 50/200 witness rows, an external 975 latency proof, a passing
975 capacity cell and the conditional 1,050 cell remain unverified.

## Candidate and verification

`candidate-1` uses fresh native release Horizon and bot prerequisites supplied
by E17, the E12 FINAL store, and E18's release proxy, sidecar and packet client.
The proxy SHA-256 is
`b8fa3896d10bfc8e334ef56bcb8aca14804cc0bc860ff7fc44516f48eaaea7c8`.
All hashes and the exact patch/source manifest are in
`data/E18/candidate-1-build.json`. The numbered delta is
`patches/meridian-05-proxy-cost.patch`. Clean application and complete source
byte equality passed against the frozen E12 stack.

Scoped formatting and strict release Clippy passed. Release tests passed 202
config, 33 ghost, 93 handoff, 562 proxy, 99 sidecar and two signal cases. The one
known listener-burst test was explicitly skipped by its exact name. An earlier
attempt used the wrong skip name and retained its macOS backlog failure.

Four drain tests prove ordered compressed/encrypted delivery, bounded size-cap
flushes, completed overlapping drains and no replay after a write failure.
The complete sidecar suite found a pooled fake-endpoint teardown problem that
the six focused tests missed. Pool entries now have generations; a late borrower
cannot resurrect a retired entry. The fake accepts idle concurrent connections
but still serializes command execution. Its 500 ms timeout/recovery assertions
remain unchanged, and all 99 tests pass. A targeted late-return regression was
added. The interrupted old full run and its SIGTERM outcome remain in the data.

## Commands so far

```sh
python3 -B scripts/e18-proxy --bots 50 --seconds 20 --name before-50-profile --sample
python3 -B scripts/e18-proxy --bots 200 --seconds 20 --settle 0 --name before-200-profile --sample
python3 -B scripts/e18-patch --event-loop
python3 -B scripts/e18-gates --name reactor-gates-5 --event-loop --build
python3 -B scripts/e18-freeze --name reactor-1 --event-loop --flat09 --gates reactor-gates-5
python3 -B scripts/e18-check proof --bins .local/bin/meridian-e18/reactor-1 --crossings 1000 --name reactor-crossings-1000
python3 -B scripts/e18-check terrain --bins .local/bin/meridian-e18/reactor-1 --crossings 100 --name reactor-wall-terrain-100
python3 -B scripts/e18-check items --bins .local/bin/meridian-e18/reactor-1 --name reactor-items
python3 -B scripts/e18-check borders --bins .local/bin/meridian-e18/reactor-1 --name reactor-owned-borders
python3 -B scripts/e18-check live --bins .local/bin/meridian-e18/reactor-1 --name reactor-live-state
python3 -B scripts/e18-check teleport --bins .local/bin/meridian-e18/reactor-1 --name reactor-no-input-teleport
python3 -B scripts/e18-check lifecycle --bins .local/bin/meridian-e18/reactor-1 --name reactor-fast-lifecycle
python3 -B scripts/e18-physics --snapshot .local/bin/meridian-e18/reactor-1 --variant e12 --suite --name reactor-physics-off
python3 -B scripts/e18-physics --snapshot .local/bin/meridian-e18/reactor-1 --variant e12 --suite --particles --name reactor-physics-on
python3 -B scripts/e14-build10-check docs/experiments/data/E18/physics/reactor-physics-off.json.gz docs/experiments/data/E18/physics/reactor-physics-on.json.gz --snapshot .local/bin/meridian-e18/reactor-1 --output docs/experiments/data/E18/physics/reactor-verification.json
python3 -B scripts/e18-production-slope --name production-D9-capacity-diagnostic-3 --capacity-diagnostic --counts 975,1050 --warmup 30 --seconds 60 --bins .local/bin/meridian-e18/reactor-1
```

The first command completed. The second returned failure and retains its full
pre-termination warning list. No failed sample contributes to a steady-load
acceptance table. A formatting command initially used paths relative to the
wrong working directory and ran no test; its corrected command is separate.

All listed reactor gates and correctness commands completed successfully. The
physics checker retained one unresolved timing window. The final diagnostic
command completed but failed its capacity gates and skipped 1,050. Derived
capacity summaries retain the raw-file hash and do not alter the raw samples.
One physics analysis invocation used E14's default derived output path. Its
previously clean tracked bytes were restored exactly, Git blob
`b648310058775cd669a2d339e2457190fc63eae2`, and the analysis was rerun with an
explicit E18 output. No raw E14 evidence was overwritten.

The patch-05 D9 release 50/200 proxy rows and patch-05/06 capped 975 diagnostics
are measured. A passing M4 result is not. The source stack is byte-equality
checked and retained binaries are outside disposable targets. No Folia, Paper,
GPL source or excluded research was read. No E18 commit or push was made.

## Handoff and cleanup

The final patch-06 stack applies cleanly after frozen patch 05. Every source
byte still matches the frozen `reactor-1` manifest, and every retained binary
hash matches its candidate record. The generated flat-light scratch was removed
after its proof. The hung sidecar test trace was compressed and retained before
its scratch copy was removed. Home prefixes in four older sample reports were
mechanically redacted with before/after hashes in
`data/E18/home-prefix-redactions.json`; no sample or numeric measurement was
dropped. The final result-tree home-path scan is clear.

All E18 fixtures, run directories and quiet markers are gone. Plain
`scripts/clean-builds` completed, without `--all`; the 566 MiB Meridian target
and 38 MiB root target were below the configured cap. Retained binaries and
small evidence remain. No active E18 build or load is left running.
The next goal needs to address the measured manager polling and IO latency,
then repeat correctness and capacity. This experiment does not establish M4.

## Published evidence

Before publishing, the reviewer replaced the names of processes outside the fixture with `outside` in every raw row that sampled the whole host (41 files under `data/E17/` and `data/E18/`). Their PIDs and CPU figures are unchanged, so the noise rule can still be checked; fixture names such as `horizon-client`, `meridian` and `python3` are kept. File hashes recorded before that step no longer match the published copies. The two physics screenshots, which show game textures, and `pooled-200-profile-2-instruments.json`, an Instruments trace that holds the host environment, stay local under `.local/evidence/E18/`.
