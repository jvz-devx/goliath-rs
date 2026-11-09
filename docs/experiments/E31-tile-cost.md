# E31. Why a tile's tick costs what it does

Status: analysis from retained data and source. The builder ran no load,
cluster or build, and made no patch. Every estimate below needs the
reviewer's measurement before it counts. Nothing was committed or sent
upstream. No GPL source or excluded research was read.

## Question

A tile at about 106 players fails the raw tick P99 rule where plain Horizon
holds 200 on one main thread. Why does the tile's tick cost more, and which
changes would raise the cluster frontier most cheaply?

## Short answer

Three findings change how the question should be read.

1. At the same player count per process a tile's main thread needs about
   25 to 30% more CPU than plain Horizon, not twice as much. On this
   machine a tile at 106 players uses 19.7 ms of main-thread CPU per tick.
   Plain Horizon interpolates to about 15.5 ms at 106.
2. The tile does not fail because its main thread runs out of CPU. At the
   failing 475 step the main thread uses 0.49 core. Its ticks last 32.9 ms
   on average, which is 0.66 core of wall time. At least a quarter of each
   tick is spent off the CPU. Plain Horizon shows none of that: at 200
   players its tick time and its main-thread CPU agree within 1%.
3. Plain Horizon's own cost grows with the square of the players in the
   process. The largest single cause in the source is the locator bar's
   waypoint manager, upstream code that runs on both sides. It explains why
   plain Horizon stops at 200 and why splitting players over tiles helps so
   much. It is also the largest main-thread item on a tile.

So the frontier has two levers. Off-CPU time in the tick tail is one. CPU
per tick, mostly upstream, is the other. Our own additions come third.

One correction to the brief: a tile at 106 players serves 42 terrain
readers in the 425 cell, not 65. Readers are 10% of the total, so 65 is the
650-player figure from the Mac.

## Accounting

### Linux, D9 layout, pinned

Per tile, averaged over the builder's and the reviewer's v3 cells (16 tile
rows per step). "CPU per tick" is main-thread cores times 50 ms. "Off-CPU"
is one minus main-thread CPU over summed raw tick time. It is a lower
bound, because main-thread CPU also includes the control work between
ticks.

| Step | Placed | Readers | Main cores | CPU per tick ms | Tick mean ms | P50 ms | P99 ms, mean / worst | Off-CPU, at least |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| 400 | 100.1 | 40 | 0.358 | 17.9 | 19.31 | 18.71 | 32.0 / 43.3 | 7.4% |
| 425 | 106.3 | 42 | 0.393 | 19.7 | 22.27 | 21.51 | 37.4 / 40.8 | 11.7% |
| 450 | 112.6 | 45 | 0.438 | 21.9 | 26.06 | 24.84 | 48.2 / 53.0 | 16.0% |
| 475 | 118.9 | 47 | 0.486 | 24.3 | 32.85 | 30.47 | 68.5 / 76.8 | 26.0% |

Plain-09, same layout in one process:

| Players | Main cores | CPU per tick ms | Tick mean ms | P50 ms | P99 ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 50 | 0.146 | 7.3 | 7.31 | 7.23 | 8.74 |
| 150 | 0.504 | 25.2 | 25.23 | 25.18 | 29.20 |
| 200 | 0.829 | 41.4 | 41.62 | 41.29 | 46.69 |
| 225 | 0.996 | 49.8 | 51.11 | 50.54 | 56.78 |

What these rows support:

- Plain Horizon's tick is flat. P99 is 1.13 times P50, and almost all of
  the gap is one heavier tick in every 20 (about 4 ms extra at 200, visible
  when ticks are folded by tick number modulo 20). The tile has the same
  20-tick bump, about 2.5 ms.
- The tile's tick is not flat. P99 is 1.7 to 2.2 times P50.
- Plain cost fits `5.9 - 0.02 N + 0.001 N^2` ms within 0.5 ms at all four
  counts. The linear term is about zero. Cost per player doubles between
  100 and 200 players.
- From 400 to 475 the tile's CPU per tick rises 36% for 19% more players,
  which is the same square law. Its tick wall time rises 70%.

### Where the tile's P99 comes from

The lite rows record main-thread CPU for every cycle on the final stack at
425. Joined with the raw tick durations of the same tile:

| Row, tile | Ticks | Band | Wall ms | Cycle CPU ms | Wall minus CPU ms |
| --- | ---: | --- | ---: | ---: | ---: |
| b1, a | 1,230 | all | 25.41 | 20.70 | 4.71 |
| | | lower half | 22.41 | 19.21 | 3.20 |
| | | top 1% | 40.29 | 29.77 | 10.53 |
| b2, b | 1,223 | all | 21.37 | 19.91 | 1.47 |
| | | lower half | 18.61 | 18.12 | 0.49 |
| | | top 1% | 37.47 | 29.66 | 7.81 |

A P99 tick of 38 to 40 ms at 425 is about 30 ms of CPU and 8 to 10 ms of
not running. Both halves matter. The median tick needs 19 ms of CPU, so the
tail also has 10 ms of extra CPU whose cause the data doesn't name.

Handovers are not that cause. Ticks with three or more activations
anywhere in the cluster within one tick cost at most 1.8 ms more CPU than
ticks with none (b1: 22.43 against 20.66 ms; b2: no difference). Wall minus
CPU doesn't follow activations either.

Tile processes take 475 to 770 involuntary context switches a second in
the 425 lite controls, about 25 to 38 per tick. The retained data has no
run-queue delay per thread, so it can't say whether the off-CPU time is
waiting for a CPU or blocking. The four server CPUs are 65 to 82% busy at
these steps, with about 975 proxy threads, 8 tile network threads and 4 cut
writers beside the 4 main threads. Plain Horizon has the same four CPUs to
itself. The Mac shows the same shape with a different mechanism. E20's
slow-tick spans record 46.3 ms of wall time against 17.8 ms of CPU, and
D12 records that the limiter there turns the CPU budget into pauses.

### Matched count, same layout (Mac)

| Row | Players | Readers | Handovers | Main cores | P50 ms | P99 ms |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| Plain, one square (E21) | 163 | 0 | no | 0.258 | | 15.3 |
| Tile, identical work (E23, t1 r1) | 162 | 0 | no | 0.300 | 14.6 | 18.1 |
| Tile, D9 at 650 (E26 A rows) | about 162 | 65 | yes | 0.326 | 16.3 | 41.3 |

With no readers and no crossings a tile costs 16% more main-thread CPU than
plain at the same count, on the stack before patch 10. Patch 10 later
removed about 0.016 core of warming, which leaves about 10%. Adding the
crossing cohort and 65 readers adds 0.026 core, 9%, and doubles P99. The
four-tile identical-work rows use 2.5 of the 4 cores and keep P99 at 20 to
21 ms. The D9 rows use 3.3. The tail appears when the cluster nears its
core budget, not when the tile gets more work.

### What the full timers attribute to our patches (Mac, 650, FINAL-09)

4,845 cycles on the selected tile, mean busy time 24.08 ms. The timers
inflate main CPU by 22.6% (E26), so every share of ours is an upper bound.

| Bucket | ms per cycle | Share | Calls per cycle | Covers |
| --- | ---: | ---: | ---: | --- |
| Upstream work | 16.77 | 69.6% | | everything not ours |
| Hub additions ("observer control") | 2.40 | 10.0% | 1,981 | every hub hunk of ours: per-packet authority gates, the wall hold in the move path, reader radius lookups |
| Ownership checks | 2.14 | 8.9% | 66,444 | chunk and area tests. At 32 ns a call this is mostly timer cost |
| Warming | 0.93 | 3.9% | 18 | measured alone at 0.857 ms, then 0.058 ms with patch 10 |
| Control and transfer | 0.75 | 3.1% | 5,439 | import, export, activation |
| Cut state | 0.43 | 1.8% | 7,070 | revisions, capture, enqueue |
| Lease gate | 0.37 | 1.5% | 6,860 | |
| Reader tracking and chunks | 0.23 | 1.0% | 7,357 | observer-only delivery |
| Status, digest, measurement | 0.03 | 0.1% | | digest unarmed |

Upstream work is 67 to 71% in every duration band, the slowest 2% of ticks
included. In that slowest 2% (62.6 ms busy) upstream work is 42.0 ms
against 12.5 ms in the faster half. Control and transfer rises to 8.4 ms
there, the only bucket of ours that grows faster than the tick. Warming
falls to 0.29 ms.

### Where the profile is silent

- No retained profile splits upstream work on Linux. The only function-level
  evidence is the Mac stack sample of E21, which counts occupancy, not CPU.
- The hub bucket can't be split between its sites.
- Nothing separates run-queue waiting from blocking inside a tick.
- Nothing names the extra 10 ms of CPU in the tile's slowest ticks.
- The readers' cost was measured once, on the Mac, on the original engine:
  0.025 core of main thread per tile for 65 readers (E21 contained pair).

## Source findings

Paths are under `crates/horizon-client/src/` in `.local/src/horizon-tiles`,
the final tile tree at the pin. "Upstream" means the file is byte-identical
in the plain tree.

### 1. Locator-bar waypoints: about 3N^2 scans per tick (upstream)

`game/lan_waypoints.rs:31` runs once per tick, from `game/lan.rs:2874`. It
rebuilds a `Player` for every placed guest and calls
`Waypoints::tick_level`. Readers are not included (`:34` filters on
`placed`).

For every player whose position changed, which is every moving player,
`lan/waypoints.rs:536-558` calls `update_waypoint` once and `update_player`
twice. Each of those loops over all players (`:401-409`, `:432-437`) and
calls `update_or_create` (`:361`). That is about 3N^2 calls a tick. Each
call does:

- a lookup in `connections`, a std `HashMap<u128, Vec<..>>` with SipHash
  (`:147`, `:365`);
- a linear scan of the receiver's row for the source (`:368`). Every player
  transmits to every other at `PLAYER_RANGE`, so a row holds N - 1 entries;
- two more SipHash lookups in `Tick::pair` (`:239`), and a fourth in
  `chunk_visible` (`:210`) for chunk and azimuth connections;
- one or two square roots, and an `atan2` for pairs more than 332 blocks
  apart (`:229-236`, `:300`).

At 200 players that is 120,000 calls a tick over rows of 199 entries. At
106 it is 33,700 calls over rows of 105.

Evidence that this is the square term: it is the leading non-waiting leaf
on the tile's main thread in E21's sample (657 of 3,482 non-waiting
samples, with SipHash writes another 257), E20 already recorded it as an
upstream lead, and the plain fit has a pure N^2 shape with 0.33 µs per
call if all of it were waypoints. The fit bounds the tile's share from
above at 6.7 to 8.4 ms of its 19.7 ms at 106. The Mac leaf share puts it
no lower than about 4 ms. On plain at 200 the same reasoning gives 20 to
35 ms of 41.

The rule `locator_bar` switches all of it off: `update_or_create` and
`create_connection` return on their first line when it is false (`:344`,
`:362`, read at `game/lan_waypoints.rs:65`).

Other upstream work that grows with the square of players, smaller in the
same sample: `building_players` (`game/lan_actions.rs:336`) with
`BuildingPlayers::for_turn` (`game/building_players.rs:20`), 126 samples,
and `Guest::bounds`, 67.

### 2. Entity tracking treats every reader as a viewer (upstream loop, our sessions)

`Hub::track_entities` (`lan/hub.rs:3485`) builds its viewer list from
`placed || held` (`:3550`). A terrain reader is a held guest, so it is a
viewer. For each viewer `Entities::tick` makes a flag pass over every
entity with a hash lookup per entity (`lan/entity.rs:1137`, `:1181-1201`),
and for each entity with changes it clones the encoded packets into every
tracking viewer's queue (`:1286-1296`).

A reader near a wall therefore tracks the players and mobs in its sent
chunks and receives their movement every tick. The proxy drops all of it:
its terrain filter passes terrain packets only, pinned by the test
`entity_and_player_state_are_never_terrain`
(`meridian-handoff/src/terrain.rs:660`). The tile pays for the flag pass,
the clones, the network thread's write, and the proxy pays to read and
discard.

### 3. The move path reads the environment for every movement packet (ours)

`Hub::position_change` calls `std::env::var_os("GOLIATH_STORE")` at
`lan/hub.rs:4458` for every accepted movement packet, and again at `:4565`
when the wall hold changes the position. Each call takes the process
environment lock, scans the environment and allocates a copy of the value.
`tile_ownership::area()` beside it is already cached in a `OnceLock`
(`crates/horizon-core/src/tile_ownership.rs:68`). Other uncached lookups sit on
handover paths only (`game/dedicated/transfer.rs:278`, `:461`, `:491`,
`:499`, `:607`, `lan/import.rs:520`, `lan/hub.rs:1232`).

The per-packet authority gate at `lan/hub.rs:4005` calls
`authority::may_act()` once per inbox packet. It is a fencing check and
stays.

### 4. Reader recentering runs outside the packet hold (ours)

`player-observe` (`game/dedicated/transfer.rs:105`) runs from
`poll_server_control` (`game/dedicated/status.rs:45`) between ticks. It
sets `g.center = None`, which makes the next tick recompute that reader's
whole view, and with a radius it sends `SetChunkCacheRadius` outside the
tick's `Holding` scope, which costs one wake system call. Every `player-*`
request also formats and writes a log line (`transfer.rs:32`). The
proxy recentres a reader when its chunk changes or every 5 s, so this is
about one request a tick per tile in D9. Small.

### 5. Things that are already right

- Packets a tick sends are held and handed over once per player, with one
  wake per network thread (`lan/conn.rs:441`, called at `game/lan.rs:2883`).
- Warming is admitted once per world tick since patch 10.
- A tile starts two network threads on four CPUs
  (`lan/io_pool.rs:265-267`), so the cluster adds 8 of them to the same
  four CPUs as the 4 main threads.

## Ranked candidates

Ranked by expected main-thread saving per unit of risk. Savings are
estimates until measured.

### 1. Take the locator-bar cost out of the tick

Mechanism: section 1 above. Two forms.

- **1a, configuration.** Set `gamerule locator_bar false` on both sides. No
  build. A sharded world can't show a correct bar from one tile anyway,
  since a tile knows only its own players. It belongs with chat and the
  tab list in the proxy (D8), or stays off.
- **1b, code.** Keep the feature and make `update_or_create` constant time:
  an index from source to row position beside each row, and the crate's
  `FxHashMap` for `connections`, `Tick::players`, `Tick::view` and `memo`.
  About 60 lines in `lan/waypoints.rs`. Rows keep their order, so the
  packets and their order don't change.

Expected saving on a tile at 106 players: 4 to 8 ms of CPU per tick with
1a, about 70% of that with 1b. At a P99 to P50 ratio near 2 that is 8 to
16 ms of P99, one to two steps of 25 players, before counting the lower
load on the shared CPUs (0.3 to 0.6 core over four tiles).

What could break: nothing in ownership, fencing, cuts or terrain. Waypoints
only emit packets and never write world state, so the digest fixture is
unaffected. For 1b the risk is packet order towards clients. The three
unit tests in the file cover joins, leaves and team colours, not ordering
under churn, so 1b needs a test that drives the old and the new manager
with the same random join, move and leave sequence and compares the output
packet for packet.

The catch is the comparison, not the code. This is upstream work that the
plain baseline also does, and plain gains more: by the fit, 20 to 35 ms of
its 41 ms at 200. D9 requires the same engine and workload on both sides,
so the baseline must be measured again with the same change. The absolute
cluster frontier should rise. The ratio to the baseline will probably
fall, because much of today's 2.1x comes from dividing this square term by
four. The reviewer should decide with that in view.

How to test: cells Q1 and Q2 in the plan.

### 2. Stop the main thread waiting inside ticks

Mechanism: the tick tail holds 8 to 10 ms of off-CPU time at 425 and at
least 26% of all tick time at 475, on CPUs shared with the proxy's 975
threads. No engine change. First measure what the waiting is (cell D1).
If it is run-queue delay, the cheapest fix is scheduling inside the same
four-CPU budget: start the proxy, store and sidecars at a positive nice
value so the four main threads win wake-ups, or lower only the tiles'
network threads and cut writers.

Expected saving: up to the full 8 to 10 ms of P99 at 425 to 450, which is
about one step. No CPU is saved, so the core budget still caps the result
near 500 to 525.

What could break: proxy latency, which has its own 5 ms P95 rule and was
tight before. D12 defines the budget as cpusets only, so thread priority
is a new deployment setting and needs a decision. No risk to ownership,
cuts, terrain or the digest.

How to test: D1, then Q3.

### 3. Leave terrain readers out of entity tracking

Mechanism: section 2. In `Hub::track_entities` and `Hub::note_joined`, skip
guests in `terrain_observers` when building viewers. Held arrivals that
are real players keep their entities. Two sites, about ten lines, tiles
only, so the baseline is untouched.

Expected saving: bounded above by E21's whole reader cost, 1.3 ms of main
thread per tick for 65 readers on the Mac. Tracking is part of that, so
0.3 to 1 ms per tick at 42 readers is a fair guess, plus network-thread
and proxy CPU for packets the proxy now reads and drops.

What could break: a reader that someone later promotes to a real player
would start with no tracked entities. Today a reader can never be imported
or placed (`game/dedicated/transfer.rs:310` refuses the import), so this
can't happen, and the patch should assert it. Terrain past the wall is unaffected, because chunks,
light and block changes use other paths. Sounds and level events still
reach readers and are still dropped by the proxy.

How to test: a hub unit test (a reader beside a moving mob gets no
`AddEntity`, a held arrival does), the E12 terrain and border checks, the
crossing checks, then Q4.

### 4. Cache the cluster-mode test in the move path

Mechanism: section 3. Read `GOLIATH_STORE` once into a `OnceLock<bool>` and
use it at `lan/hub.rs:4458` and `:4565`. The environment is fixed at
start, so behaviour can't change.

Expected saving: unknown and small. The Mac hub bucket of 2.4 ms is the
ceiling and includes timer cost and every other hub site. A lookup costs on the
order of a microsecond and runs about once per placed player per tick, so
0.1 to 0.3 ms per tick is likely. Worth doing with candidate 3 in one
patch, not alone.

What could break: nothing. Tests: gates, then Q4 with candidate 3.

### 5. Not a change yet: name the tail's extra CPU

The slowest ticks need 30 ms of CPU against 19 at the median, and no
retained data says why. Horizon's own slow-tick spans
(`lan_sync_world`, `lan_chunks`, `lan_block_changes`, `lan_track_entities`,
`lan_player_state`, per-guest packet notes at `lan/hub.rs:2433-2456`) can
name it at no build cost. Cell D2.

## Prototype

None. The two candidates that can move the frontier need a decision first
(candidate 1 changes the baseline, candidate 2 changes D12's budget
rule), and both have a test that needs no build. Candidates 3 and 4 are
safe but too small to show in a frontier cell, so a gated patch with no
load to reproduce would add nothing yet.

`.local/src/horizon-tiles` was read only. It is unchanged: pin
`fc2868b2b`, 60 status lines, the same before and after. No stack was
added to `scripts/boat-gates` and no boat run was started.

## Measurement plan

All cells use the existing runners, pinned, 30 s warm-up, 60 s measured,
D9 layout, direct bots. Diagnostics carry no pass rule. Names are
suggestions.

**D1. Run-queue delay, no build.** Extend the thread counter in
`scripts/goliath_platform.py` (it reads only the first field of
`/proc/<pid>/task/<tid>/schedstat`) to keep the second field, run-queue
delay in ns, and per-thread context switches, for each tile main thread.
Then two cells at 425 and two at 450 with the final bundle:

```sh
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E31 --cpu-mode pin --direct-bots --counts 450 --name e31-rundelay-450-r0
```

Read, per tile: summed raw tick time, main on-CPU time, main run-queue
delay. If run-queue delay covers most of tick time minus CPU, candidate 2
is live. If not, the main thread blocks inside ticks and the next step is
a stack sample of the blocked main thread.

**D2. Slow-tick spans, no build.** One cell at 450 with Horizon's span
recording enabled on the tiles and the slow-tick lines kept. Group the
lines of ticks above 40 ms by span. This perturbs timing, so it names the
work and gives no number for the frontier.

**Q1. Locator-bar ablation, no build.** Add `gamerule locator_bar false`
to the rule lists at `scripts/e17_d9_baseline.py:86` and
`scripts/e20-frontier:316` behind a switch. Plain-09 at 200, two repeats.
Final bundle at 425, two repeats. Prediction: plain main-thread CPU falls
from 0.83 core to between 0.13 and 0.43, and tile main CPU from 0.39 to
between 0.23 and 0.31. If plain falls by less than 0.08 core the waypoint
reading is wrong and candidate 1 is dead.

**Q2. Frontier with candidate 1.** Only if Q1 confirms and the reviewer
accepts a new baseline. Bundles: plain-09 and final, both with the same
form of candidate 1 (rule off, or patch 1b on both trees). Plain
staircase from 200 in steps of 25, and cluster staircase from 425 in
steps of 25, two repeats a step. Unchanged rule: 20 TPS, raw P99 under
50 ms, two passes at a step and two fails one step above. The saving that
counts is a closed cluster bracket above 425, reported with its new
baseline and the new ratio.

**Q3. Priority variant, no build.** Same final bundle, with the proxy,
store and sidecars started at nice 5. Two cells at 450 and two at 475.
Pass rule as in Q2, and the proxy latency rule must hold in the same
variant: one latency cell at the new frontier with E25's stamps, internal
and external P95 as before. Counts only if 450 passes twice.

**Q4. Candidates 3 and 4 as one patch.** Bundle `final` against
`final` plus `horizon-24`, with lite builds of both. E26b's fixed order
A/B/B/A then B/A/A/B at 425, eight cells, `scripts/e26-lite-cell` for A
and `scripts/e26b-final-lite-cell` for B. The saving counts if B minus A
in selected-tile main cores is negative by more than the within-arm range.
On this machine that range was 0.0054 core over four A rows, so a saving
of 1 ms per tick (0.02 core) is visible and 0.3 ms is borderline. Raw P99
ranges over 8 ms between like rows, so P99 can't qualify this patch. Gates
before any cell: the hub unit test, E12 terrain and border checks, the M2
crossing checks, the digest fixture.

Order: D1 and Q1 first. They cost six cells, need no build, and decide
between the two large candidates.

## Unproven

- The waypoint share on Linux. It rests on a Mac occupancy sample, the
  shape of the plain fit and the source. Q1 settles it.
- That off-CPU time is run-queue delay. D1 settles it.
- The cause of the extra CPU in the slowest ticks.
- Every saving figure above, and every frontier forecast. The step
  estimates assume P99 moves about twice as far as CPU per tick, which is
  the observed ratio, not a law.
- That the core budget caps the cluster near 500 to 525 once the tail is
  fixed. It extrapolates 3.3 cores at 475 with the square law.
- The effect of candidate 1 on the ratio to the baseline. The fit says
  plain gains more than the tiles. Only a new baseline shows by how much.

## Method notes

Numbers come from `data/E28/frontier`, `data/E28/reviewer-frontier`,
`data/E28/baseline`, `data/E28/reviewer-baseline`, `data/E28/lite`,
`data/E26/e26-ab-b*.json.gz`, `data/E23/identical-*`, and
`data/E21/baseline-plain163-1.json` and its stack summary. Quantiles are
nearest-rank over the retained raw ticks. The lite join matches each
cycle's `tick` to the raw tick with the same `game_time` on the selected
tile. No retained file was changed.
