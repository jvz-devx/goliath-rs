# Progress

The source of truth for where the project stands. Decisions and their reasons are in `decisions.md`.

How to keep this file:

- Tick an item only with evidence beside it: a command result, a report in `docs/experiments/`, or a commit.
- The reviewer ticks items after verifying them. The builder builds and reports.
- Add a line to the log at the bottom whenever something changes.
- If an item turns out to be wrong, change it here and say why in the log.

## Current state

Working on M1. Automatic crossings, a real fenced store and a correct but slow tile cut exist. E6 (fast cut), E7 (tile ownership) and E8 (fast crossing) are running. E3, E4, E5 and E9 are verified.

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
- [x] Crossing a wall triggers the handover with no command typed. Verified: E4, 1,000 of 1,000 crossings in two runs, all eight borders and the diagonals, no command typed.
- [x] Optional visible wall: the proxy draws particles near a tile wall when enabled, off by default, with no cost when off, proven by a client in and out of range. Verified: 720 packets in range, 0 out of range, 0 when off, in two runs. Rendered look in a stock client is unverified.
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
- [ ] Store group commit: commit P95 under 20 ms at 64 concurrent clients with `F_FULLFSYNC`. E5 measured 223 ms.

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
- [ ] Capacity and latency numbers come from optimised release builds, not the `fast` development profile.
- [ ] The proxy adds under 5 ms P95 per packet at the target player count, with its CPU and thread count reported. Meridian uses 3 blocking threads per player today.
- [ ] A tick profile of the busiest tile is recorded, and nothing we added costs over 5% of the tick.
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
- 2026-11-07: Built E5's local durable store and shared fence/CAS kernel, pending review. The 1,000-kill run and every-byte tail checks passed, and the disabled-fence control failed as required. Full-barrier commit P95 was 3.954, 34.080 and 222.896 ms at 1, 8 and 64 clients. E1's sweep hash is unchanged. Evidence: `experiments/E5-store.md`. No milestone boxes ticked.
- 2026-11-07: Optional particle wall added to M1.
- 2026-11-07: Completed E4, pending review. Four real engines, all eight directed borders and four diagonal directions, 1,000/1,000 final crossings with no disconnect or inventory change. Particle-off run received zero particle packets. Input gap P50/P95/P99 was 177.100/247.261/251.105 ms. Optional blue dust passed near/far/off delivery checks, about 9.0 KiB/s at a two-wall corner. Missing-trigger fault failed as required; the earlier dead-player refusal and unrelated Rust-suite failures remain in the report. Evidence: `experiments/E4-wall-crossing.md`. No milestone boxes ticked.
- 2026-11-07: Speed pass lines added: store group commit, release builds, proxy overhead and tick cost of our own additions.
- 2026-11-07: Completed E3, pending review. The final binary passed 200/200 SIGKILL/restart trials plus stale-fence and unloaded-footprint rejection checks. Normal-50 request-to-install P95 was 160.2 ms; the approach pre-cut's frozen delta was 169.7 ms. Internal durable work P95 was at most 119.5 ms, so dispatch/queueing remains part of the failure. A lower-load normal-one-bot repeat also missed. Evidence: `experiments/E3-coherent-cut.md`. Build cleanup freed 1,847 MiB; observed `.local` peak was 3,113 MiB. No milestone boxes ticked, no commit or push.
- 2026-11-07: Implemented E9, pending review. Group commit passed the final 1,000-kill, stale-fence, every-byte torn-tail and early-ack file-cache controls. Full-barrier commit P95 at 64 clients was 18.233 ms with the Rust driver but 35.398 ms with the original Python driver, so the store latency milestone remains open. P5 had zero duplicates, losses, owner violations or stale writes in the 1,200,006-scenario sweep. Applying 40 ms crash aborts to all policies exposed P3 duplicates; the sweep reports that failure rather than hiding it. Instant-abort P0-P4 rows still reproduce E1 byte for byte. Every permanent mutation was caught. Evidence: `experiments/E9-group-commit-and-p5.md`. No milestone boxes ticked, no commit or push.
- 2026-11-07: Built E8, pending review. Each of four cumulative Meridian configurations completed 1,000 walking crossings, plus a separate earlier 1,000-crossing stage-4 run. The final rebuild measured input gap P50/P95/P99 129.569/146.954/170.060 ms with no client arrival correction, disconnect or inventory change. Real death/respawn, E4 edge probes, abandoned preparations and live block sequences passed. The 80 ms handoff share is still missed before adding D6. A five-minute four-engine clock run with 128 bots showed up to 72 ticks of incremental drift. Scoped Rust checks passed except E2's existing formatting failure; full tick-wait attribution and physical momentum remain unverified. Evidence: `experiments/E8-fast-crossing.md`. Cleanup freed 4,150 MiB; sampled shared-project peak was 5,918 MiB. No milestone boxes ticked, no commit or push.
- 2026-11-07: E3, E4, E5 and E9 verified by the reviewer. The E3 cut missed its budget, so D6 now uses P5, a log-structured cut. Store group commit stays open until a quiet rerun.
