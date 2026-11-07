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

## M1. Two tiles and a wall

Two real Horizon servers own the two halves of one coordinate space. A player walks through the wall with a whole-view swap.

- [ ] Coherent tile-wide cut built and timed on Horizon, under load, with chunks, containers, drops and every resident player.
- [ ] Each server refuses to simulate or write outside its tile.
- [ ] Crossing the wall triggers the handover with no command typed.
- [ ] One shared store holds player records, with the policy chosen in D6.
- [ ] 1,000 consecutive crossings by a scripted walking player with no disconnect and no inventory change.
- [ ] Both tiles restart with their world and every player intact.
- [ ] `scripts/e2-real-transfer --build` run end to end from a fresh checkout.

## M2. Prewarmed crossing

- [ ] Terrain past the wall is visible and read-only.
- [ ] The handover starts on approach and momentum carries through.
- [ ] Input gap P95 under 150 ms and P99 under 300 ms, measured on loopback with a walking player.
- [ ] Riding players cross with their vehicle.

## M3. Crashes and partitions

- [ ] Killing a tile at any handover step gives zero duplicated items, and every loss is accounted for.
- [ ] A partitioned tile can't keep writing after losing ownership.
- [ ] Results from real processes match what E1 predicted.

## M4. Scale

The baseline is still undecided, see the open questions.

- [ ] 1, 2 and 4 tiles measured with identical bot load, seed and hardware.
- [ ] At least 1.5 times the baseline player capacity with four tiles.
- [ ] 20 TPS and tick P99 under 50 ms at that load.

## Later

- Dynamic tile ownership (goal 2).
- Shared edge band for redstone, mobs and items (goal 4).

## Open questions

- Tile size.
- Who owns global state: time, weather, scoreboard, chat.
- Where the Nether and End live.
- Scaling baseline: today's single-threaded tick, or Horizon's planned parallel tick.
- What "zero divergence" means, since Horizon isn't deterministic today and has no world digest.
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
