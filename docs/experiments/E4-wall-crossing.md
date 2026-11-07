# E4. Walking through walls

2026-11-07. Implementation evidence, pending review.

## Result

The final run completed 1,000 of 1,000 crossings with zero disconnects, zero
inventory changes and zero particle packets with the option off. All eight
directed borders and all four diagonal directions were exercised, with no game
command typed. The pre-particle run also completed 1,000 of 1,000 crossings.

| Final metric, 1,000 moves, particles off | P50 | P95 | P99 |
| --- | ---: | ---: | ---: |
| Server-observed input gap, ms | 177.100 | 247.261 | 251.105 |
| Total wall switch, ms | 696.948 | 764.371 | 802.490 |
| Largest arrival correction, blocks | 0.320 | 0.588 | 0.588 |

The final command was `python3 -B scripts/e4-walls --edge-cases`, exit 0 in
1,243.201 s including setup, separate edge probes and cleanup. Its offline
verifier passed. Its recorded runner and patch hashes equal the files on disk.

The earlier default-difficulty soak completed 66 crossings, then Horizon refused
crossing 67 because the player was dead. Its results are retained, not discarded.
No disconnect or inventory change occurred in those 66 completed moves. Inventory
after the failed death attempt was not measured.

| Pre-particle metric, 1,000 completed moves | P50 | P95 | P99 |
| --- | ---: | ---: | ---: |
| Server-observed input gap, ms | 194.430 | 249.040 | 286.004 |
| Total wall switch, ms | 696.974 | 765.302 | 794.383 |
| Largest arrival correction, blocks | 0.320 | 0.588 | 0.588 |

The 150 ms target belongs to M2. It was not met here and was not used to filter
samples. The check with tile routing deliberately omitted returned exit 2 on
the first missing crossing, with zero disconnects.

| Run | Completed / attempted | Outcome |
| --- | ---: | --- |
| Original default difficulty | 66 / 67 | Refused because the player was dead; stopped |
| Pre-particle peaceful soak | 1,000 / 1,000 | Passed |
| Final peaceful soak, particles off | 1,000 / 1,000 | Passed, zero particle packets |
| Planted missing-trigger fault | 0 / 1 | Expected runner failure |

These are kinematic protocol measurements on loopback. The test player sends
positions at 5.6 blocks/s, with a nominal 50 ms cadence. It accepts teleports and
continues towards its waypoint. It does not run Minecraft movement physics.

Four engines run separate flat worlds with seed 12345 and identical coordinates.
This is spatial routing in Meridian, not tile ownership in Horizon. Each engine
still simulates and saves outside its assigned tile. There is no shared store,
fencing, coherent checkpoint, terrain composition or crash-safety claim here.

## Revisions and scope

| Component | Revision |
| --- | --- |
| Retained Horizon engine with E2 input tracing | `fc2868b2b26d56ec76e17414909fe58a3b87bb92` |
| Meridian base | `902dcf195e0a8bbd3f39a5f560684af00e64bf5d` |
| Meridian protocol/world dependency | `0d21d5520ec26c49b774bf1533e53ea8af81278d` |
| Prepared game resources | Nightly `2026-11-05-66f3981`, Minecraft 26.3 |

Meridian starts with `patches/e2-meridian-client.patch` applied. E4 adds
`patches/meridian-01-wall-trigger.patch`. The E4 patch changes only Meridian's
configuration, session position observation, crossing arrival setting, proxy wall
routing, tests and a new `e4-client` binary. It does not change the E2 client.
The patch was checked and applied to a clean archive of the pinned base after E2.
Applied sources compared equal to the implementation checkout.

E4 uses `.local/bin/horizon-fc2868b2b/horizon-client` and
`.local/bin/meridian-902dcf1/meridian-sidecar`. New proxy and client binaries are
retained in `.local/bin/meridian-e4/`. Binary SHA-256 values are in each result.
Horizon source and `crates/goliath-store` were not changed. No commit, push, issue,
comment or PR was made. No Folia, Paper, other GPL source or
`docs/research/05-folia.md` was read.

## Crossing rule

`[tiles]` has an integer `origin`, integer block `edge`, and an `entries` list
mapping each `[x,z]` tile index to one backend. The experiment uses edge 64,
origin `[0,0]`, and these four entries:

| Backend | Tile | Coordinate interval |
| --- | --- | --- |
| a | `[0,0]` | x and z in `[0,64)` |
| b | `[-1,0]` | x in `[-64,0)`, z in `[0,64)` |
| c | `[-1,-1]` | x and z in `[-64,0)` |
| d | `[0,-1]` | x in `[0,64)`, z in `[-64,0)` |

For each axis, floor the client coordinate to a block, subtract the grid origin,
then use `div_euclid(edge)`. At origin zero this is exactly `TileGeometry`'s rule.
For example, x = -0.01 belongs to tile -1, x = 0 belongs to tile 0, and x = -64
belongs to tile -1. Non-finite coordinates and coordinates outside the 30 million
block bound do not select a destination. Unknown tiles retain the current backend
and produce a deferred notice. Configuration rejects missing sidecars, unknown
backends, duplicate tile indexes, duplicate backend owners, edge lengths below 2,
a default backend with no tile, and disabling carried positions.

The owner changes only when a position is at least 0.5 blocks inside the new tile
on both axes. This margin is a trigger rule, not a different geometry rule.
Standing on a wall, jittering within the margin, or walking exactly along a wall
keeps the current backend. A diagonal position can select the diagonal backend
directly. No intermediate cardinal transfer is required.

The handoff crate observes position-bearing movement packets outside its session
locks. Rotation-only packets and teleport acknowledgements never trigger routing.
The proxy owns the worker. There is at most one wall worker per player, and one
handoff at a time. Its selected position is fixed for that handoff, so later input
cannot redirect its arrival to a different tile.

While a handoff is running, movement updates one deferred position. After commit
the worker checks it again and switches if its eligible tile differs from the
current backend. A reversal therefore completes the first handoff, then returns.
A second crossing is deferred. Multiple changes coalesce to the latest position;
this is deliberately not a FIFO of every intermediate tile, nor an input replay
queue. A position in the wall margin waits for later eligible movement. The
counted itinerary waits for each requested crossing, so coalescing cannot conceal
a missing counted move.

An upstream refusal stops automatic routing for that session and tells the player
why. It does not retry indefinitely or pretend the player arrived. Existing
upstream handoff abort, commit and failure behavior is otherwise unchanged.

## Runner and measurements

`scripts/e4-walls` reuses E2's child process, engine startup, control, NBT inventory
decoder and accepted-input helpers. It does not edit E2's scripts. Each run starts
four engines, four sidecars, one proxy and one test client on loopback ports picked
at runtime. Every run has fresh game worlds, `HORIZON_HOME`, launcher scratch and
XDG directories under `.local/runs/E4-*`. It stops its processes and deletes that
run folder on exit, including failed runs.

Prepared files are read-only at:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

The runner uses offline synthetic player `E4Probe`, view and simulation distance 3,
seed 12345, survival mode and, for the final soak, peaceful difficulty. Mob
spawning, weather changes and random ticks are disabled. The known inventory is
five diamonds and twelve oak logs. The checksum is SHA-256 of sorted-key JSON of
the decoded inventory NBT list, the same definition as E2. Before and after every
counted move it must equal the initial checksum, with 17 items.

The final inventory checksum was
`c200b92c38f59972717cc23bc2460c8b571c7864080f16e01454c6b3751a8e2f`
before and after every one of the 1,000 moves.

Waypoints are at x and z = +/-1. Starting on a, the repeated itinerary is:

```text
a -> b -> c -> d -> a -> d -> c -> b -> a -> c -> a -> d -> b -> d -> a
```

The first eight moves cover every directed cardinal border. The later moves cover
both directions of both diagonals. Reversals appear in the route, and separate
probes reverse or cross again while the first switch is still in flight.

- Input gap is the first target accepted-movement timestamp minus the last source
  accepted-movement timestamp, from `E2_TRACE_INPUTS`. It is not a proxy timer.
- Total time is the first eligible client position send to the successful wall
  arrival chat receipt. It includes the proxy and sidecar scheduling, plus commit.
- Arrival correction is the largest Euclidean displacement from the client's
  current position to either absolute arrival teleport, using E2's observation
  window through 100 ms after the first target input.
- Percentiles use nearest rank, `ceil(p*n)`, with no interpolation.

The runner checks the expected count, indexes, owner chain, route, accepted target
tile, inventory checksums, counts and timestamp arithmetic. It also compares the
actual arrival-message sequence with the requested route. Missing, duplicated or
unexpected handoffs fail the run. The offline verifier recomputes corrections and
percentiles. The client has no `ChatCommand` send path. Its stdin `walk` orders are
local waypoint instructions, not Minecraft commands. Setup commands run on the
server console, not on the player connection.

## Commands and checks

From a clean Meridian `902dcf1` checkout with the E2 patch already applied:

```sh
cd "$REPO/.local/src/meridian"
git apply --check "$REPO/patches/meridian-01-wall-trigger.patch"
git apply "$REPO/patches/meridian-01-wall-trigger.patch"
env -u CI CARGO_INCREMENTAL=0 CARGO_PROFILE_FAST_INHERITS=release \
  CARGO_PROFILE_FAST_LTO=false CARGO_PROFILE_FAST_CODEGEN_UNITS=16 \
  cargo build --locked --profile fast -p meridian-proxy --bin meridian \
  -p meridian-testkit --bin e4-client
mkdir -p "$REPO/.local/bin/meridian-e4"
cp target/fast/meridian target/fast/e4-client "$REPO/.local/bin/meridian-e4/"
cd "$REPO"
python3 -B scripts/e4-walls --crossings 14 --name pilot
python3 -B scripts/e4-walls --crossings 14 --edge-cases --name edge-pilot
python3 -B scripts/e4-walls --edge-cases
python3 -B scripts/e4-walls --crossings 1 --fault-skip --timeout 3 --name fault-skip
python3 -B scripts/e4-walls --verify docs/experiments/data/E4/trials.json
python3 -B scripts/e4-runner-test
scripts/clean-builds --all
```

All Cargo commands use `CARGO_INCREMENTAL=0`, one `fast` profile and Meridian's
single workspace-local `target/`. Builds are local because the binaries run on
macOS. No Boat home exists for this project. No Horizon build was started by E4.

The first build failed on a borrowed-config versus `Arc<Config>` mismatch. It was
fixed. Clippy initially rejected an eighth argument in the switch helper. Grouping
the destination and carried position fixed it without a routing change. The final
soak uses binaries rebuilt after this fix.

Scoped tests ran with:

```sh
cd "$REPO/.local/src/meridian"
# Use the same environment and profile settings as the build above.
cargo test --locked --profile fast -p meridian-config -p meridian-handoff \
  -p meridian-proxy --lib --bins
cargo clippy --locked --profile fast -p meridian-config -p meridian-handoff \
  -p meridian-proxy -p meridian-testkit --all-targets -- -D warnings
cargo fmt --all --check
cargo test --locked --profile fast --workspace --quiet \
  -- --skip serve::tests::the_listener_queues_a_large_burst
```

The scoped suite passed 201 configuration tests, 80 handoff tests and 556 proxy
tests. One unchanged proxy test failed, `serve::tests::the_listener_queues_a_large_burst`.
At `crates/meridian-proxy/src/serve.rs:645` it reported
`refused after 128 connections: connection timed out`. This test was explicitly
excluded from the later workspace run, not fixed or counted as a pass.
Final scoped Clippy passed. `cargo fmt --all --check` failed only on E2's existing
unformatted `e2-client.rs`, which was left intact. E4's touched Rust files were
formatted separately. The five offline runner tests passed, including mutations
for a skipped crossing, a wrong actual arrival, inventory changes, bad timestamps,
wrong corrections, bad percentiles and a disconnect.

The broader workspace attempt also returned exit 101. It reached the unchanged
panel integration test `the_whole_bench_starts_servers_sidecars_proxy_and_stops_in_reverse`,
which timed out waiting for a fake backend. That fake process had exited with
`AddrInUse` on port 20050. Later workspace targets were not reached, so a complete
workspace pass is not claimed. These two test failures are separate from real
wall crossings, which allocate their loopback ports at runtime.

## Failed run and limits

`failed-survival-66.json` preserves the first soak. It used the original fixture's
default difficulty. The proxy received a real sidecar refusal, `that player is
dead`, at the freeze for a to d, after 66 completed moves. It sent a refusal and
latched automatic routing off. The client stayed connected on the source.
The runner returned exit 2 rather than accepting an unperformed crossing.
Existing hostile mobs are a possible explanation, not a separately measured cause
of death. The fixture changed to peaceful difficulty for the next run. Both runs
remain visible in the report and raw data.

| Default-difficulty soak, 66 completed moves and one failed attempt | P50 | P95 | P99 |
| --- | ---: | ---: | ---: |
| Accepted-movement input gap, ms | 205.277 | 241.430 | 487.537 |
| Total switch, ms | 706.439 | 786.561 | 1,034.450 |
| Largest arrival correction, blocks | 0.320 | 0.588 | 0.588 |

The failed attempt has no arrival or input-gap measurement. It is counted as a
failure, not a zero-time move. The two 14-crossing pilots are separate samples in
`pilot.json` and `edge-pilot.json`. They are not mixed into a 1,000-crossing table.
All completed samples are retained without trimming. These short waypoints bound
arrival corrections, so their maximum is not comparable to E2's unbounded straight
walk as proof of improved movement continuity. No prewarming was added.

No terrain ownership, interaction replay, real physics client, stock client,
vehicles, WAN, loaded-server limit, backend crash, partition or fenced durability
was tested. Healthy inventory equality does not establish crash safety. Horizon's
existing separate-player-file authority limits from E2 still apply.

## Optional visible wall

The same Meridian patch can draw a cosmetic wall with clientbound particle
packets. It is off by default. This draws no blocks and changes no Horizon state.

```toml
[tiles.wall-particles]
enabled = true
particle = "dust"
colour = 0x3388ff
spacing = 1
distance = 8
```

`particle` supports `dust` and `end_rod`. Dust uses a 24-bit RGB `colour` and scale
1. End rods have no colour field. `spacing` is whole blocks, from 1 to 8.
`distance` is the perpendicular distance to the wall, from 1 to 32 blocks.
Defaults are blue dust, colour `0x3388ff`, spacing 1 and distance 8.

The proxy draws only the part of a nearby wall within four blocks along it, from
0.2 to 3.2 blocks above the player's feet. A burst has at most 128 particles. The
movement-driven hook sends at most one burst per 500 ms, with no timer thread.
It honours the client's particle limiter and distance setting. It is a visual
marker, not a claim that the backend enforces ownership or collision there.

When disabled, hook construction returns the original wall-routing callback.
There is no per-move visual branch, clock read, drawing geometry or timer. The
extra counter reference for byte accounting is captured once at session setup.
This is a source-based no-drawing-work guarantee, not a CPU microbenchmark.

The 26.3 codec carries dust as particle id 21 with `ColorScale`, and end rods as
id 27 without extra data. These ids were checked against the pinned MIT protocol
dependency and the prepared 26.3 registry. No Horizon checkout changes were needed.

Measured delivery with the final proxy and test client:

| Option | Player x,z | Window, s | Received particle packets | Framed particle bytes | Extra TCP payload bytes/s |
| --- | --- | ---: | ---: | ---: | ---: |
| On, near two walls | 1,1 | 5.001358 | 720 | 46,080 | 9,213.498 |
| On, out of range | 16,16 | 5.004819 | 0 | 0 | 0 |
| Off, near | 1,1 | 5.005022 | 0 | 0 | 0 |
| Off, out of range | 16,16 | 5.005033 | 0 | 0 | 0 |

The near client decoded every packet as dust, colour `0x3388ff`, count 1, with
positions on x = 0 or z = 0. Ten bursts each had 72 packets and 4,608 bytes. The
byte count is the connection's `bytes_out` delta under the client writer lock,
so it excludes unrelated traffic and includes the protocol frame/compression
overhead. It excludes Ethernet, IP and TCP headers, as E2 does. This is about
9 KiB/s per player at the measured corner with two walls visible, not a one-wall
or loaded-server benchmark. Out-of-range drawing stopped completely. Already
created particles may remain visible until the client expires them.

Commands, all exit 0:

```sh
python3 -B scripts/e4-walls --particle-evidence --wall-particles
python3 -B scripts/e4-walls --particle-evidence
python3 -B scripts/e4-walls --verify docs/experiments/data/E4/particles-on.json
python3 -B scripts/e4-walls --verify docs/experiments/data/E4/particles-off.json
python3 -B scripts/e4-runner-test
```

The eight offline runner tests also reject changed particle delivery and byte
counts. Updated scoped Rust tests passed 202 configuration, 80 handoff and 559
proxy tests, with the known listener-burst test explicitly skipped. Final scoped
Clippy passed. The actual rendered appearance in a stock client is unverified;
this evidence proves packet delivery and payload colour, not a screenshot.

## Retained evidence and disk

`docs/experiments/data/E4/` is about 5 MiB:

- `trials.json`: final 1,000 moves, accepted source/target inputs, teleports,
  correction distances, inventory hashes, actual arrival sequence and zero
  particle packet count. Runner, patch and binary hashes are included.
- `pre-particles.json`: the first successful 1,000-move run, before the optional
  particle addition, with its own binary hashes and statistics.
- `failed-survival-66.json`: 66 completed moves and the dead-player refusal on
  attempt 67. Its diagnostic tail is retained; no full game log is kept.
- `pilot.json`, `edge-pilot.json`: separate 14-move pilots, plus the explicit
  standing, parallel-walking, jitter and in-flight route-change evidence.
- `fault-skip.json`: deliberately omitted routing, rejected by the runner.
- `particles-on.json`, `particles-off.json`: near/far windows, decoded client
  samples and per-burst measured framed byte counts.
- `verification.json`, `cleanup.json`: gate outcomes, known failed checks,
  protocol provenance, sampled disk use and cleanup outcome.

No world, exported player NBT, prepared resource contents or account data is
retained in the results. Runtime worlds and logs were deleted after each run.
Both full runs reached 22 MiB in their run folders at shutdown. The largest
observed shared `.local/` sample was 3,105 MiB. Meridian's target reached 560 MiB.
These are sampled high-water values, not continuous peak tracking; temporary
compiler files may have made the true peak larger. Other agents built under
`.local/` at the same time, so its shared total is not attributable to E4 alone.

`scripts/clean-builds --all` returned exit 0. It deleted the idle 378 MiB Meridian
target and reported the project at 2,526 MiB. It correctly skipped the 1,802 MiB
Horizon target because another agent was building there. No active build was
forced clean. E4's run directories and processes are gone. Retained binaries in
`.local/bin/meridian-e4/` remain. No source, other agent work or game resources
were removed. The deleted build output and run fixtures are disposable and can
be regenerated with the recorded commands.

## Independent verification

The reviewer reran the full run on 2026-11-07 with `python3 -B scripts/e4-walls --edge-cases --name repro`, and recomputed the numbers from raw trials. The machine was busy with other agents' builds.

| Metric | First run | Reproduction |
| --- | ---: | ---: |
| Crossings completed | 1,000 of 1,000 | 1,000 of 1,000 |
| Inventory equal after every move | 1,000 | 1,000 |
| Disconnects | 0 | 0 |
| Input gap P50 / P95 / P99, ms | 177.1 / 247.3 / 251.1 | 209.5 / 242.5 / 322.6 |
| Total switch P50 / P95 / P99, ms | 697 / 764 / 802 | 710 / 784 / 850 |

The particle check also reproduced: 720 packets near the wall, 0 out of range, 0 with the option off, and 9,205 bytes per second per player against 9,213. Raw data is in `data/E4/repro*.json`.
