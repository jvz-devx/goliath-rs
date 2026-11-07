# Progress

The source of truth for where the project stands. Decisions and their reasons are in `decisions.md`.

How to keep this file:

- Tick an item only with evidence beside it: a command result, a report in `docs/experiments/`, or a commit.
- The reviewer ticks items after verifying them. The builder builds and reports.
- Add a line to the log at the bottom whenever something changes.
- If an item turns out to be wrong, change it here and say why in the log.

## Current state

M0 is finished and verified. Next is M1. Crossings work on real servers, but the walking input gap P95 is about 480 ms against a 150 ms target, and P2 needs a coherent tile-wide cut that Horizon does not have yet.

## M0. First evidence

Find out what a crossing costs today and what keeps items safe when a tile crashes.

### E1, crash simulator (`crates/goliath-model`, `crates/goliath-sim`)

- [x] The same seed twice gives an identical trace hash, shown by a test. Verified: `just test` passes, and a rerun of the sweep gave a byte-identical CSV.
- [x] A failing seed replays from one command and prints a minimised trace. Verified: `just replay P1 6 0` printed a 3-event reduced schedule, hash `d5f674e5e5771983`.
- [x] Disabling fencing makes the checker fail, shown by a test. Verified: test in `crates/goliath-model/tests/it.rs`, plus the `late_abort_unfenced` mutation.
- [x] `just sweep` runs at least 1,000,000 scenario-seeds and writes a results table. Verified: 1,000,005 scenario-seeds, `E1-results.csv`, SHA-256 `e70e7e27...` reproduced.
- [x] P0 shows duplication in that sweep, so a zero elsewhere means something. Verified: P0 failed 65,650 of 66,667 seeds at setting 0, P1 failed 24,572.
- [x] `docs/experiments/E1-durability.md` names a recommended policy and the checkpoint latency it needs for an input gap P95 under 150 ms. Verified: P2, under 128 ms of checkpoint work at 8 ms fsync.
- [x] The reviewer reviewed the model and ran the rust-project-architecture audit. Verified: audit clean, and 4 planted P2 bugs caught. See "Independent verification" in the E1 report.

### E2, real transfer numbers

- [x] Horizon at `fc2868b2b` and Meridian at `902dcf1` built, with binaries in `.local/bin/`. Verified: binaries present and used by the reproduction.
- [x] Horizon's `tools/server_transfer_e2e.py` result recorded. Verified: 37 of 37 checks, recorded in `data/E2/`, and the reproduction reran it.
- [x] 50 stationary and 50 walking moves measured between two real servers. Verified: 100 of 100 in both runs.
- [x] Input gap, total time, arrival error, velocity and bytes reported at P50, P95 and P99. Verified: walking gap P95 495 ms, reproduced at 477 ms. The 150 ms target is not met yet.
- [x] Inventory checksums match before and after every move. Verified: recomputed from raw trials, 100 of 100 equal in both runs.
- [x] One command in `scripts/` reproduces the run. Verified: `python3 -B scripts/e2-real-transfer` with retained binaries, 131 s. The fresh `--build` path is not verified.
- [x] `docs/experiments/E2-real-transfer.md` lists defects and the changes a prewarmed crossing needs. Verified: two defects and three proposed changes.
- [x] The reviewer reproduced the headline numbers independently. Verified: see "Independent reproduction" in the E2 report, `data/E2/repro-trials.json`.

### Close-out

- [x] Durability decision recorded as D6 in `decisions.md`. Done: P2, with conditions.
- [x] Peak disk use reported, and `scripts/clean-builds --all` run. Done: peak about 3 GiB in `.local`, target dirs 1,965 and 435 MiB. E1 reached 95 MiB. Cleaned after the final gates.
- [x] Verified work committed and pushed. Done: pushed to origin/main with this update.

## M1. Four tiles and walls

Four real Horizon servers own a 2 by 2 grid of one coordinate space. A player walks through any wall with a whole-view swap.

- [ ] Tile size, the owner of global state (time, weather, scoreboard, chat) and Nether and End placement decided from a short experiment and recorded in `decisions.md`.
- [ ] Coherent tile-wide cut built on Horizon and timed under load, with chunks, containers, drops and every resident player.
- [ ] Each server refuses to simulate or write outside its tile, proven by a test that tries to.
- [ ] Unloaded chunks past a wall can't read as air to simulation code, proven by a test.
- [ ] Crossing a wall triggers the handover with no command typed.
- [ ] One shared store holds player records, using the policy chosen in D6 with a fence on every CAS.
- [ ] 1,000 consecutive crossings by a scripted walking player with no disconnect and no inventory change.
- [ ] All four tiles restart with their world and every player intact.
- [ ] `scripts/e2-real-transfer --build` run end to end from a fresh checkout.

## M2. Prewarmed crossing

- [ ] Terrain past the wall is visible, read-only, with no blank chunks at the wall during a crossing.
- [ ] The handover starts on approach and momentum carries through, with no backward correction above 0.1 blocks.
- [ ] Input gap P95 under 150 ms and P99 under 300 ms over 1,000 walking crossings on loopback, including the D6 checkpoint.
- [ ] A real physics client, Horizon's own under `horizon-ctl`, walks through a wall and keeps its speed.
- [ ] A stock Java Edition 26.3 client crosses a wall, confirmed by the maintainer in person.
- [ ] Riding players cross with their vehicle.

## M3. Crashes and partitions

- [ ] 1,000 crossings with a tile killed at random, covering every step: write, fsync, install, CAS and acknowledgement. Zero duplicated items, and every loss accounted for.
- [ ] A partitioned tile can't write once it has lost ownership, proven with real processes.
- [ ] A crashed tile returns and takes players again with no manual step, while the other tiles keep ticking.
- [ ] Results from real processes match what E1 predicted, and any difference is explained.

## M4. Scale and behaviour

The baseline is one plain Horizon process with the same seed, view distance, bot behaviour and total CPU limit. The bots run with their CPU limited separately, so they can't be the bottleneck without it showing.

- [ ] `horizon-bots` can walk set paths and survive a backend switch, using a patch or a wrapper.
- [ ] A tick digest compares a tile with plain Horizon, and a redstone, piston and TNT fixture inside one tile ends identical.
- [ ] 1, 2 and 4 tiles measured with identical load, with mob counts reported so less gameplay work can't pass as speed.
- [ ] Four tiles hold at least 3 times the baseline's player capacity at 20 TPS with tick P99 under 50 ms. The spec's floor was 1.5 times.
- [ ] A 30-minute soak at that load with crossings running shows no duplicates, no owner violations and no growth in memory beyond 10%.
- [ ] Baseline rerun against Horizon's parallel tick if it has landed by then. If it hasn't, say so.

## Later

- Several physical machines, with latency and partitions on a real network.
- Dynamic tile ownership (goal 2).
- Shared edge band for redstone, mobs and items (goal 4).

## Open questions

- Project name, since Goliath is DonutSMP's name for its system.
- How much of the research can be published.

## Log

- 2026-11-07: Research finished. D1 to D3 decided. Further design questions moved to experiments.
- 2026-11-07: Public repo created, MIT. Research notes kept local.
- 2026-11-07: E1 and E2 started. Build cleanup in place.
- 2026-11-07: 26.3 game files prepared through the launcher, which unblocked E2.
- 2026-11-07: Completed E2, pending review. 100/100 real Meridian moves, matching inventory checksums, walking input gap P95 495.296 ms. Evidence: `experiments/E2-real-transfer.md`. Build targets cleaned; no milestone boxes ticked.
- 2026-11-07: E1 finished. The reviewer reproduced the sweep byte for byte and caught 4 planted P2 bugs. D6 chose P2.
- 2026-11-07: E2 finished. The reviewer reproduced it: 100 of 100 moves, walking gap P95 495 ms and 477 ms. M0 closed.
- 2026-11-07: M1 to M4 tightened into pass lines. Baseline fixed as plain Horizon at equal CPU, target raised to 3 times, divergence defined as a tick digest on a fixture.
