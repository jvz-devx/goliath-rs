# Design decisions

Design decisions, newest last. The research behind them is kept local for now. It was taken at Horizon `fc2868b2b` and Meridian `902dcf1` on 2026-11-07.

## D1. Purpose

Prove the system first with bots and invited friends. Release it as open source later so others can replicate and refine it. The intent is to reproduce the Goliath system. A public server launch is not a goal yet.

## D0. Roles

A reviewer does research, orchestration and verification. A separate builder does the implementation. An item is ticked only after the reviewer reproduces its result.

## D2. Boundary model

Wall first, halo later. Reproduce Goliath's reported behaviour: static tiles, player handover at the wall, global services outside the tiles. Ownership and gateway interfaces must leave room for a shared edge band. Goals 2 (dynamic ownership) and 4 (cross-boundary mechanics) move to later phases.

## D3. Crossing experience

Prewarmed crossing with terrain visible. Real terrain past the wall, read-only, merged by the proxy. Handover starts on approach and momentum carries through. The metric is the input gap, the time the player can't act, with P95 under 150 ms. Build in stages, with Meridian's whole-view swap as the first working step. Needs an observer mode in Horizon. Entities past the wall stay out of scope until the halo phase.

## Open questions moved to experiments

On 2026-11-07 the project moved from up-front design to learning by building. Open questions are settled by experiment and raised only when the data needs a decision.

- E1 (T0 simulator): which commit and recovery policy keeps item duplication at zero when a tile crashes and rolls back after a player left it.
- E2 (real Horizon): what a crossing costs today on real servers, for a moving player.

Still undecided: tile size, global state ownership, Nether and End placement, the scaling baseline, the redefined divergence target, naming and licence for the open-source release.

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

**Global state.** E8 measured four independent tile clocks drifting up to 72 ticks, 3.6 seconds, with one tile under load. That is a visible sky step at a wall, so tiles can't each own time. The store holds one cluster clock and the weather, and the proxy sends clients the cluster time so the sky doesn't jump at a wall. Chat and the tab list live in the proxy. Scoreboard, statistics and advancements travel in the player's record. Game rules are cluster configuration, set at start.

**Nether and End.** Each runs as one extra process with the whole dimension as a single tile, using the same handover through portals. They aren't split, and they aren't part of the M1 to M4 measurements. DonutSMP reportedly hosts them in one region as well.


## D9. Capacity layout and equal CPU

The M4 comparison gives both sides the same budget: 4 cores for the servers, with bots on a separate core. On the cluster side the 4 cores also cover the proxy, the store and the sidecars. Plain Horizon connects without a proxy, but its 325-bot frontier was limited by its single main thread, at about 1.0 core, so the proxy budget doesn't change its number. The baseline is measured on the same engine patch as the tiles (`horizon-09-flat-light`, E18). The old W1 frontier on the unpatched engine is history only.

Both sides run the same world workload. On the cluster, 90% of the players patrol four squares of the same size, one per tile, each centred at least 300 blocks from a wall. The other 10% walk a route that crosses walls all the time, including a corner. Each tile has 64 pigs, with the same 256 total on both sides. Per-tile player, reader and mob counts are reported in every cell. The plain baseline runs exactly this layout in one process: the same four squares at the same coordinates, the same crossing route (with no wall to cross) and the same 256 pigs. Otherwise each cluster player would see fewer neighbours than a baseline player, which is less gameplay work. W1's single-square frontier (325 on the patched engine) stays as a reference, not the 3x target.

Why not the baseline's square at the origin: on the cluster that is the four-tile junction, where every player sees all four tiles and each tile holds a reader for every player. That's the worst case, not how players spread out on a large map. It's measured as a separate stress row and never used as the pass line.

A cell is valid when load is 4 or less at its start and no process outside the fixture averages more than 0.5 core over the cell. Load from the fixture's own threads doesn't void a cell.

## D10. Capacity result: report the frontier, keep 3x as the aim

E19 showed that 975 players don't fit in the 4-core budget on this Mac, even with the proxy outside it. Four tiles, the store and the sidecars used 3.96 cores at 975 and still missed 20 TPS (tick P99 84 to 90 ms), and the reviewer reproduced that. Each tile costs about 1 core at about 244 players, where a plain server with that many players needs about 0.7. The difference is the terrain readers and the cluster features.

So M4 reports the cluster's measured frontier inside the D9 budget and layout, as a multiple of the matched baseline (325), with the same pass rules: 20 TPS, raw tick P99 under 50 ms, two passes and two failures per step. 3x stays the target. The budget, the layout and the pass rules don't change; the work goes into making tiles and the proxy cheaper, and the frontier is measured again after each cut.
