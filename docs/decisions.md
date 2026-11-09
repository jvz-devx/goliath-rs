# Design decisions

Design decisions, newest last. The research behind them is kept local for now. It was taken at Horizon `fc2868b2b` and Meridian `902dcf1` on 2026-11-07.

## D1. Purpose

Prove the system with bots and invited players. The code is public under MIT. The intent is to reproduce the Goliath system. A public server launch is not a goal yet.

## D0. Roles

A reviewer does research, orchestration and verification. A separate builder does the implementation. An item is ticked only after the reviewer reproduces its result.

## D2. Boundary model

Wall first, halo later. Reproduce Goliath's reported behaviour: static tiles, player handover at the wall, global services outside the tiles. Ownership and gateway interfaces must leave room for a shared edge band. Goals 2 (dynamic ownership) and 4 (cross-boundary mechanics) move to later phases.

## D3. Crossing experience

Prewarmed crossing with terrain visible. Real terrain past the wall, read-only, merged by the proxy. Handover starts on approach and momentum carries through. The metric is the input gap, the time the player can't act, with P95 under 150 ms. Build in stages, with Meridian's whole-view swap as the first working step. Needs an observer mode in Horizon. Entities past the wall stay out of scope until the halo phase.

## Open questions moved to experiments

On 2026-11-07 the project moved from up-front design to learning by building. Open questions are settled by experiment and raised only when the data needs a decision.

- E1 (deterministic crash simulator): which commit and recovery policy keeps item duplication at zero when a tile crashes and rolls back after a player left it.
- E2 (real Horizon): what a crossing costs today on real servers, for a moving player.

The later decisions settle the licence, tile size, global state, dimensions and capacity baseline. Naming remains open.

## Provenance rule

Keep the local Folia research notes and any GPL source away from implementation agents. Horizon's CONTRIBUTING bars reading GPL source, and bars sending code an AI tool wrote from it, for anything sent upstream. The reviewer read Folia during research, so upstream PRs must be written by an agent that hasn't seen that report, and the PR must disclose what was read.

## D4. Public repository

The project is public and MIT licensed, matching Horizon and Meridian. Research notes stay local until they have been reviewed for publishing.

## D5. Rust and disk rules

Follow the `rust-project-architecture` rules, recorded in `AGENTS.md`. Build output is disposable and capped by `scripts/clean-builds`.

## D6. Durability

Use P2: freeze the player, make a coherent tile-wide checkpoint durable, then commit the player's generation with a fenced compare-and-swap. E1 found zero duplicates, losses and stale writes across 200,001 scenarios per policy, while the same sweep caught P0 and P1. Mutation tests confirmed that the ordering, the single coherent cut and the fence on every CAS are each required.

This is a modelled result, so it holds only if real Horizon delivers what the model assumes:

- Horizon must produce one coherent cut of chunks, containers, drops and every resident player. Its separate saves don't qualify, and the mutation that models them fails.
- The checkpoint runs inside the frozen window. E1's budget is under 128 ms of checkpoint work at 8 ms fsync. E2's stationary gap is already about 230 ms before any checkpoint, so M2 has to shorten the commit sequence as well as prewarm it.
- If the measured cut doesn't fit, the fallback is P4, a grouped durable tick log, which also had zero duplicates but costs every action some wait. A checkpoint taken on approach, leaving only a small delta at the freeze, is worth trying first.

P3 is rejected. Its fast first route needs a cluster-wide rollback domain and embargoes further crossings.

## D7. Visible wall

Walls are invisible by default, so a crossing goes unnoticed. An optional particle wall, drawn by the proxy, can show the boundary, like the blue wall players report on DonutSMP. It helps explain why mobs and other players vanish at that line.

## D6 update

E3 built the cut as a full re-serialisation and missed the budget: 160 ms to durable and up to 94 ms of tick stall at 50 bots. The cost was the prototype's capture path, not disk flushes, which take about 4 ms. D6's rule stays, but its implementation changes to P5: capture references to dirty state at the freeze tick, append one delta to a per-tile log, and flush once. E9 found zero duplicates and losses for P5 across 200,001 scenarios, and caught every planted bug. E6 tests whether it meets the budget on real Horizon. If it can't, the fallback is still P4.

P3 is now rejected on safety as well: with crash notices 40 ms late, it duplicates items.

## D8. Tile size, global state, Nether and End

**Tile size.** Production tiles default to 2,048 blocks, a multiple of the 512-block region file so each file has one writer. The edge is configurable, and tests use 64 so crossings happen quickly. Crossing load falls with tile size: 50 sprinting players cross about 0.17 times a second at 2,048 blocks, against 1.4 at 256, using the mean rate N * v * 4 / (pi * L). At 256 blocks an E3-style 94 ms stall would take 13% of the tick thread. E3 also showed a cut's cost follows loaded chunks, not tile area, so large tiles cost nothing extra. Tiles can't be too large either, because a tile is the unit of capacity: every player in one tile shares one tick thread.

**Global state.** E8 measured four independent tile clocks drifting up to 72 ticks, 3.6 seconds, with one tile under load. That is a visible sky step at a wall, so tiles can't each own time. The store holds one cluster clock and the weather, and the proxy sends clients the cluster time so the sky doesn't jump at a wall. Chat and the tab list live in the proxy. Scoreboard, statistics and advancements travel in the player's record. Game rules are cluster configuration, set at start. This is the agreed division of responsibility; each feature needs its own implementation evidence, and E10 lists proxy chat, the cluster tab list and scoreboard transfer as not yet implemented.

**Nether and End.** Each runs as one extra process with the whole dimension as a single tile, using the same handover through portals. They aren't split, and they aren't part of the M1 to M4 measurements. DonutSMP reportedly hosts them in one region as well.


## D9. Capacity layout and equal CPU

The M4 comparison gives both sides the same budget: 4 cores for the servers, with bots on a separate core. On the cluster side the 4 cores also cover the proxy, the store and the sidecars. Plain Horizon connects without a proxy, but its 325-bot frontier was limited by its single main thread, at about 1.0 core, so the proxy budget doesn't change its number. The baseline is measured on the same engine patch as the tiles (`horizon-09-flat-light`, E18). The old W1 frontier on the unpatched engine is history only.

Both sides run the same world workload. On the cluster, 90% of the players patrol four squares of the same size, one per tile, each centred at least 300 blocks from a wall. The other 10% walk a route that crosses walls all the time, including a corner. Each tile has 64 pigs, with the same 256 total on both sides. Per-tile player, reader and mob counts are reported in every cell. The plain baseline runs exactly this layout in one process: the same four squares at the same coordinates, the same crossing route (with no wall to cross) and the same 256 pigs. Otherwise each cluster player would see fewer neighbours than a baseline player, which is less gameplay work. W1's single-square frontier (325 on the patched engine) stays as a reference, not the 3x target.

Why not the baseline's square at the origin: on the cluster that is the four-tile junction, where every player sees all four tiles and each tile holds a reader for every player. That's the worst case, not how players spread out on a large map. It's measured as a separate stress row and never used as the pass line.

A cell is valid when load is 4 or less at its start and no process outside the fixture averages more than 0.5 core over the cell. Load from the fixture's own threads doesn't void a cell.

## D10. Capacity result: report the frontier, keep 3x as the aim

E19 showed that 975 players don't fit in the 4-core budget on this Mac, even with the proxy outside it. Four tiles, the store and the sidecars used 3.96 cores at 975 and still missed 20 TPS (tick P99 84 to 90 ms), and the reviewer reproduced that. Each tile costs about 1 core at about 244 players, where a plain server with that many players needs about 0.7. The difference is the terrain readers and the cluster features.

So M4 reports the cluster's measured frontier inside the D9 budget and layout, as a multiple of the matched baseline (325), with the same pass rules: 20 TPS, raw tick P99 under 50 ms, two passes and two failures per step. 3x stays the target. The budget, the layout and the pass rules don't change; the work goes into making tiles and the proxy cheaper, and the frontier is measured again after each cut.

## D11. Horizon's client stands in for the vanilla client

The real-client checks run Horizon's own client, a reimplementation of the Java Edition 26.3 client, because it can be driven and measured from scripts. On 2026-11-08 the project decided to treat it as equivalent to the vanilla client for crossing behaviour and not to run a separate in-person test with a stock 26.3 client.

What that leaves unproven: vanilla has its own code for chunk loading and unloading, movement prediction and entity tracking, so a difference there could show up only in a stock client. The protocol both clients speak is the same, and every crossing check on the final stack passes with Horizon's client. The M2 item for the stock client stays unticked and says so.

## D12. Second platform: Linux with core pinning

E28 repeats the M4 measurements on a second machine, a 6-core Intel i5-8400 (no SMT, 16 GiB RAM) running Linux with cgroup v2. Its numbers are reported as a second platform beside the Mac's and never replace or correct them. Each platform's frontier is compared only with its own baseline.

D9's budget stays: four cores for the servers (tiles, proxy, store and sidecars, or the one plain Horizon process), one core for the bots. On Linux it is enforced by pinning instead of the SIGSTOP duty-cycle limiter:

- CPUs 0 to 3 run the servers, CPU 4 runs the bots, and CPU 5 runs the harness, the test-only front shims and everything else on the host. Each group is a cgroup v2 cpuset. For the length of a window the system's own slices are narrowed to CPU 5, so outside work can't run on the fixture's cores, and are restored afterwards.
- The front shims sat outside both caps on the Mac too. They stay outside them here, on CPU 5, and their CPU is reported.
- The CPU governor is `performance` during a window, turbo stays on, and each cell records per-core frequency and thermal-throttle counters. A cell whose throttle counters rise is invalid.

The reason: on the Mac, 82 to 88% of the proxy's P95-tail sends overlapped a limiter STOP (E25). The limiter turns a CPU budget into pauses that show up as latency. Pinning gives the same core count with no pauses. The limiter stays available as a comparison mode, so the effect of the change is measured on this machine.

D9's validity rules apply unchanged: load 4 or less at the start of a cell, and no outside process averaging more than 0.5 core. With six cores that rule leaves the harness core little room, so each cell reports CPU 5's total busy time and the outside total, and a cell with CPU 5 saturated (95% busy or more) is invalid. Each cell also reports swap-in, since the machine has 16 GiB.

### D12 addendum: the soak's memory line on this platform

Two things about the 30-minute soak's memory line needed a ruling once it ran on the second machine. The line itself, no process growing more than 10%, is unchanged.

- Memory is resident plus swapped. This host swaps to compressed memory under soak load, so a process's resident size can fall while its real use doesn't. The line is judged on resident plus swap for each process, and both are published.
- The baseline is taken at the later of minute 5 and the first sample after the store's receipt window is full, and the judged window runs 25 minutes from there. The store keeps a bounded window of retry receipts. At the Mac's 650 players it filled before minute 5. At this machine's 425 players it fills about 30 seconds after, so the store appeared to grow 14 to 16% and then stayed flat. Growth from minute 5 to minute 30 is still published beside the ruled figure.
