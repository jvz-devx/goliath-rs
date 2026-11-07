# E11. Measurement tools for M4

Implementation and local measurements. Review is required.
No milestone box is ticked. Nothing was committed or sent upstream.

## Tools

`patches/horizon-03-bots.patch` adds an opt-in `--event-loop` client to
`horizon-bots`. Existing modes keep their existing implementation. The new
client owns all sockets on one thread through Mio. It handles offline login,
compression, configuration, chunk acknowledgements, keep-alive, ping,
teleport acknowledgement, Respawn and Transfer. START_CONFIGURATION sends
the configuration acknowledgement and repeats configuration. World changes
clear the chunk cache. Routes and waypoint progress survive those changes.
Transfer reconnects with the transfer handshake and retained cookies.
Online-mode authentication is not implemented.

The route mode uses seeded per-bot patrols, evenly stratified over `--spread`.
`--path 'X,Z;X,Z'` sets an absolute repeating route instead, including routes
through walls. `--servers` spreads one process's connections round-robin over
several endpoints. The report includes successful joins, online clients,
movement distance, chunks, corrections, configuration switches, transfers,
chunk waits and missed 50 ms movement deadlines. It never bursts catch-up
movement after a missed deadline.

These are protocol load clients. They send positions, not physics inputs.
They do not model collision, inventory actions or survival walking.

`patches/horizon-04-digest.patch` adds opt-in measurement to Horizon.
`HORIZON_E11_MEASURE=1` enables it. `e11-samples` drains bounded raw tick
durations and digest rows from the dedicated control endpoint.
`e11-digest x,y,z,maxx,maxy,maxz once|every` selects an inclusive region in
the current dimension. `e11-digest off` stops automatic digests.

Blocks are hashed in coordinate order with SHA-256. Block entities use the
normal live save view, including moving pistons, canonical NBT compounds and
sorted records. Entities use a sorted multiset of kind and exact position
bits. Generated UUIDs and iteration order are excluded. A region with an
unloaded chunk is refused. Entity kinds outside Horizon's shared entity
visitor are refused rather than omitted silently.

The digest does not cover entity inventories, velocity, AI, scheduled ticks,
RNG state, lighting, players or every dimension at once. Matching it is
evidence for the selected fixture, not general deterministic replay or
vanilla parity. Digest collection is disabled during capacity measurements.
Raw tick timing remains enabled in both capacity configurations.

Both patches apply against Horizon `fc2868b2b` after E2. E7 can be applied
alongside them. `scripts/e11-build` temporarily reverses only the exact E7
patch in the owned checkout to build plain Horizon, then restores E7 even on
failure. `--tile` retains E7 in the built engine. Both use the existing
release profile, ThinLTO, one codegen unit and `CARGO_INCREMENTAL=0`.
There is one target directory. Retained binaries and provenance are in
`.local/bin/horizon-e11/`.

## Equal CPU

`scripts/e11_support.py` uses Darwin `proc_pidinfo` for process CPU, memory and
thread counts. CPU time includes all threads, including threads that exited.
Mach timebase conversion is required on this Mac. An independent `getrusage`
check verifies those units.

The limiter gives a group of server PIDs one aggregate CPU-seconds budget,
and gives the bot PID a separate budget. It samples every 2 ms and uses
SIGSTOP and SIGCONT with a 5 ms-per-core credit bound and hysteresis. Idle
time cannot accumulate unlimited credit. Shutdown always resumes stopped
processes. This is an experimental userspace limiter, not a cgroup or core
affinity. Stop intervals affect wall-clock tick latency and remain in the
measurements. Core placement is not fixed. This Mac reports six `Super`
cores and twelve `Performance` cores through `sysctl`, with no efficiency
core class. An efficiency-core placement policy would not apply here.

The final calibration measured 0.999 cores for one busy worker capped at one
core, and 1.001 for four workers sharing the same cap. Uncapped one-worker
CPU was 0.999 cores. Four workers under `taskpolicy -b` still used 3.934
cores. That makes QoS unsuitable as an equal CPU limit. See `cpu-probe.json`.

The first limiter revision undershot a one-worker cap at 0.858 cores because
tiny accounting skew caused repeated stops. Hysteresis fixed that in the
retained calibration. An initial probe also used the wrong path for
`taskpolicy`, which is under `/usr/sbin` here.

## Capacity method

`scripts/capacity-bench` starts fresh private worlds and homes, with unique
loopback ports, and deletes them after each cell. It never launches the
launcher or changes its files. Prepared 26.3 resources are read-only.

Default workload is seed 12345, flat terrain, view and simulation distance 3,
seeded route clients at 4 blocks per second over a 128-block square, and 64
invulnerable persistent pigs with AI enabled. Natural spawning is disabled.
The total pig count is 64, divided between servers, not 64 per server.
Bot placement happens before warmup. This is a controlled chunk, movement
and mob load, not a representative survival-player capacity estimate.

The normal run uses 30 warmup seconds and 60 measured seconds. CPU budgets
are four cores total for servers and one for bots. Four plain servers get
one spatial quarter of the bots each through the same single client process.
The harness median-splits the unchanged global route anchors by x and z,
then supplies a per-client endpoint list. It checks its route calculation
against the real client's emitted route preamble before measuring. Global
routes, seed and speed stay the same as the one-process run.
They are independent worlds, not a working tile cluster.
Each cell waits for one-minute host load at most 4 before launching. The
quiet-start wait is bounded at 15 minutes. A valid overload bracket is
bisected to 25-bot steps by default.

TPS comes from differences in `time query gametime`, using the midpoint of
each request as its timestamp. The acceptance tolerance is two ticks over
the measured window, not `1000 / MSPT`. P99 uses individual raw server tick
samples with nearest rank, not an average of overlapping rolling percentiles.
Between-tick chunk work is reflected in game-time TPS and total process CPU.

A cell cannot pass with wrong player, mob or entity counts, a tick-buffer
overflow, missing ticks, stale bot telemetry, bot CPU above 80% of its
allotment, a missed bot movement deadline, excessive waits for chunks, an
exceeded server CPU cap or disturbed host load. The default disturbance line
is one-minute load above 8 on this 18-core M5 Pro. Disturbed cells are retried.
Counts, loaded chunks, CPU and load averages are retained per tile and sample.
An invalid cell cannot establish the server's capacity ceiling.
Final validation also requires at least 95% of the offered route movement,
allowing for the client's per-waypoint step quantization. Online clients that
stop moving cannot pass. Both mean and sampled interval bot CPU must stay
below 80% of the client allowance.
The uncapped low-load limiter control is not a capacity run. It records host
load but does not reject the CPU load its own uncapped engines create.

The table reports the largest accepted tested step and the first valid
overload step. It does not pretend to know an untested exact maximum.
`--bot-only` runs the client saturation experiment and never publishes a
server capacity number.

An external-cluster configuration accepts actual engine PIDs, control
endpoints, proxy PIDs and bot-facing addresses. The proxy belongs in the
server CPU budget. `service_pids` includes a store or coordinator in that
same budget. Release build provenance and `release_verified=true`
are required. `cluster-config.example.json` is deliberately not runnable
until its placeholder PIDs and paths are replaced. `mob_origins` places
the fixed population inside each tile. The cluster owner must provide fresh
worlds and compatible spawn placement between cells.

## Commands

```sh
python3 -B scripts/e11-build
python3 -B scripts/e11-build --tile
python3 -B scripts/e11-cpu-probe --seconds 5
python3 -B scripts/e11-tools-test
python3 -B scripts/e11-capacity-round
python3 -B scripts/capacity-bench --loads 25,50,100,200,400,800 --repeat 2 --out docs/experiments/data/E11/plain-staircase.json
python3 -B scripts/capacity-bench --servers 4 --loads 100,200,400,800 --repeat 2 --out docs/experiments/data/E11/four-plain-staircase.json
python3 -B scripts/e11-fixture --ticks 80
```

## Results

### Fixture and noise floor

Plain Horizon against itself matched at all 80 stepped ticks. E7 with a
2,048-block tile and distant walls also matched all 80 ticks against plain
Horizon. Final states matched. The automatic per-tick trace agreed with
on-request snapshots in every run.

Each run observed four distinct states of the redstone clock, moving-piston
block entities, live primed TNT, its removal after the fuse and blocks changed
by the blast. A deliberately placed interior diamond block changed the
digest in all three runs. These controls passed.

This fixture disables random ticks, natural spawning, weather changes and
block drops. TNT has explicit zero velocity and no gravity. The blast has
its normal power, with a small destructible core above a bedrock floor.
The repeatable selection is deliberately small. Dropped-item positions,
random mob AI, unrestricted explosions, world generation and boundary
contraptions have not been shown deterministic by this result.

### Client switches

Four live Transfer commands alternated between a plain engine and the
retained original E7 engine. All four reconnected, completed configuration
and continued their absolute route. The final client stayed online, retained
its route and recorded no missed movement deadline. This is a real Transfer
test, not a test of the integrated Meridian seamless switch. Configuration
re-entry and Respawn-without-an-arrival-correction are covered by unit tests.
The integrated seamless proxy still needs the cluster owner's live run.

### Capacity

The reviewer arranged an exclusive E10 pause using `.local/quiet-window`.
`scripts/e11-capacity-round` waits for that marker and load at most 4,
then runs the cells below. It rejects a lost marker and releases the window
before the 45-minute deadline. All servers still use fresh worlds and homes.
In this coordinated round the four-server schedule uses one, two, three and
four times the measured baseline lower bound. Requested and effective load
steps are both stored in JSON. This spends the limited window on the scale
question rather than repeating small four-server cells.

The low-load limiter control used four plain servers, four total bots and
64 total AI-enabled pigs. Both capped and uncapped runs held 20 TPS.

| Server CPU policy | CPU cores used | Worst tile tick P99, ms | Measured group stops |
|---|---:|---:|---:|
| Four-core aggregate cap | 4.000 | 12.580 | 3,768 |
| Uncapped control | 10.916 | 0.844 | 0 |

The limiter clearly inflates wall-clock P99 in this low-load case. A capped
P99 is the service latency under this CPU allocation method, not pure
Horizon execution time. No limiter time is subtracted to manufacture a pass.
An overloaded capped result alone cannot establish an intrinsic Horizon
stall. This one control pair does not bound limiter distortion at every load.

### Plain baseline

| Total bots | Repeat | Game-time TPS | Capped tick P99, ms | Server CPU cores | Bot CPU cores |
|---:|---:|---:|---:|---:|---:|
| 50 | 0 | 20.000 | 2.406 | 3.064 | 0.011 |
| 50 | 1 | 19.999 | 2.389 | 3.051 | 0.011 |
| 100 | 0 | 19.998 | 5.691 | 2.590 | 0.024 |
| 100 | 1 | 20.000 | 6.154 | 2.584 | 0.024 |
| 200 | 0 | 20.000 | 17.199 | 2.005 | 0.058 |
| 200 | 1 | 19.999 | 17.808 | 1.996 | 0.058 |
| 400 | 0 | 14.435 | 82.382 | 1.268 | 0.126 |
| 400 | 1 | 14.404 | 79.380 | 1.266 | 0.125 |

The accepted tested baseline is 200 bots in both repeats. The first measured
overload is 400. The exact frontier inside that interval is not measured.
At 200, P99 differed by 3.54% between repeats. At 400, TPS differed by 0.22%
and P99 by 3.78%. These are two repeats, not a confidence interval.

Neither limiter stopped the server or bots during either 400-bot window.
The server used about 1.27 of its four-core allowance, so that particular
overload is not caused by SIGSTOP. Bot CPU was about 0.125 cores, peak
two-second interval CPU was below 0.136 cores, every bot stayed online, and
there were no missed client movement deadlines or chunk waits. One process
driving 400 clients is therefore measured, not extrapolated from 100 bots.

### Four independent plain servers

| Total bots | Repeat | Slowest game-time TPS | Worst tile capped P99, ms | Total server CPU cores | Bot CPU cores |
|---:|---:|---:|---:|---:|---:|
| 200 | 0 | 20.000 | 15.541 | 4.000 | 0.068 |
| 200 | 1 | 19.999 | 14.707 | 3.999 | 0.069 |
| 400 | 0 | 19.998 | 18.538 | 4.000 | 0.147 |
| 400 | 1 | 20.000 | 19.326 | 4.000 | 0.147 |
| 600 | 0 | 19.998 | 27.792 | 4.000 | 0.223 |
| 600 | 1 | 20.000 | 27.657 | 4.000 | 0.228 |
| 800 | 0 only | 19.998 | 41.661 | 4.000 | 0.336 |

All these rows passed. The first 200- and 400-bot attempts exceeded the host
load threshold and are retained as invalid. Their replacement cells passed.
They are not pooled with the accepted pair. At 600, worst-tile P99 differed
by 0.49% between repeats.

The repeated four-server reference reaches 600 bots, three times the accepted
200-bot baseline step. That makes further scaling work plausible. It does not
prove a three-times ratio of exact capacities. The baseline frontier between
200 and 400 is unrefined, these are independent worlds, and there is no proxy,
shared store, crossing checkpoint or shared clock in this reference run.

The 800-bot run passed once, with client peak interval CPU 0.347 cores and
no missed client movement deadlines. This is not promoted to a repeated
capacity result. Its second repeat was interrupted during warmup at the
coordinated deadline, before a measured window. It contributes no timing
or capacity samples.

### Work and CPU per bot

Every accepted sample had exactly 64 pigs and 64 total non-player entities.
Four-server runs had 16 pigs per process. AI stayed enabled. At the same
200-bot load, plain Horizon held 319 chunks and the four-server reference held
220, 221, 223 and 209, or 873 total. At 400, the four-server counts were
219, 207, 207 and 196, or 829 total. At 600 they were 222, 221, 224 and 224,
or 891 total. These counts were unchanged between each accepted repeat.
The four-server reference did not pass by retaining fewer chunks or mobs.
Counts do not prove identical AI histories or player-to-player visibility.

At the repeated baseline load, client CPU was 288 to 290 microseconds per
bot per wall-clock second. At 400 on one server it was 313 to 315. At the
repeated four-server 600 load it was 372 to 379. These are measured CPU
costs, not a linear extrapolation to a client ceiling. The client had one
thread in every measured run.

### Window end and remaining cells

The marker was released after 44.686 minutes from its creation, before E10's
45-minute release. The round stopped the incomplete 800-bot repeat and
removed its private run folder. No E11 engine was left running. No E10
process was signalled. The deadline stop is a budget interruption, not a
server failure or a completed disturbed timing cell.

Still unmeasured:

- Four plain servers, 800 bots, repeat 1.
- Client-only saturation steps at 400, 800, 1,600 and 3,200 bots. The live
  800-bot capacity cell proves headroom there, but the saturation ceiling
  was not reached or measured.
- Finer baseline steps between the accepted 200 and overloaded 400.
- An overloaded four-server step, so there is no measured upper ceiling.
- The integrated seamless Meridian switch, proxy cost, checkpoint cost,
  1/2/4 real tiles, a 30-minute soak and a newer upstream parallel-tick baseline.

`handoff.json` names the unfinished cells. `summary.json` is the compact
revalidated table. The original raw rows remain in the staircase files,
including the rejected high-load attempts and the old coverage diagnostic.
Nothing was interpolated into a pass, and no M4 box is ticked here.

## Verification and failures

The bot crate's 13 release-profile tests passed, including seeded spread,
configuration acknowledgement, chunk-cache reset and resumed movement.
The Python tool tests passed. They verify partial-line handling, CPU units,
and refusal of saturated bots, late bots, stationary clients and reduced mob
populations. Seven tests passed in the final run.
Scoped release Clippy passed for `horizon-bots` and the client library and
binaries with default features disabled. Workspace formatting passed after
fixing the new measurement module's declaration order. Patch application
passed in three orders, including E7 before and after E11, with byte-identical
working source in the E7 combinations.

The first engine build found a save-number API mismatch in the digest.
The corrected release build completed. The first larger bot join exposed
a write-before-connect-readiness race. It stopped before measurement and
is not a capacity result. Socket readiness now gates the first write.
The focused digest command used `--bin horizon-client` and ran zero tests.
It is not counted as test evidence. Those unit tests belong to the gameplay
library, so the intended target is `--lib`. The real three-engine fixture
comparison and its positive and negative controls did run.
The first baseline validator incorrectly required at least 15 raw ticks per
wall-clock second. That mislabelled a genuinely overloaded 400-bot server
as missing data. Coverage now uses actual game-time progress and contiguous
tick sequence numbers. `e11-summarise` rechecks retained raw rows, preserves
their original reasons and records that classification change. It does not
turn the overloaded cell into a performance pass.

No Folia, Paper, other GPL source or excluded research was read. The supplied
testing survey mentions comparison projects, but their source was not used.

## Handoff and disk

`verification.json` lists commands and their actual outcomes. No full upstream
Rust suite or vanilla parity suite was run. The experiment used the pinned
Horizon revision, not a claim about current upstream parallel ticking.

Raw JSON was compacted without removing values, from about 12 MiB to about
5.1 MiB. `data-size.json` records that rewrite. The paths in retained reports
are repo-relative or sanitized. The private run folders were deleted.

Plain `scripts/clean-builds` completed. It freed 0 MiB because targets were
under their caps. The owned Horizon target was 2,480 MiB; the sampled shared
project size at cleanup was 7,629 MiB. Idle caches belonging to E10 were not
removed with `--all`. Binaries remain in `.local/bin/horizon-e11/`.
Nothing was committed or published. Review remains required.

## Independent verification

The reviewer reran `python3 -B scripts/e11-fixture --ticks 80` on 2026-11-07. It passed again: 80 of 80 ticks identical for plain Horizon against itself and for plain Horizon against an E7 tile server, with final states equal. The result is in `data/E11/repro-fixture.json`. The capacity numbers are not reproduced yet. M4's capacity line needs the real cluster measured with this harness, and the reproduction belongs with that run.
