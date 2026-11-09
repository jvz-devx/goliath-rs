# goliath-rs

An experiment in running one Minecraft world across many server processes, in Rust, on top of [Horizon](https://github.com/horizon-rust/horizon).

The world is one shared coordinate space split into static tiles. Each tile is a separate Horizon server. A player who walks through the wall between two tiles is handed to the neighbouring server without reconnecting. Vanilla Java Edition clients connect through a proxy and need no mods.

The design copies the publicly visible behaviour of DonutSMP's Goliath system. Its code isn't public, so nothing here comes from it.

## Status

This is experimental, not a production-ready server. Four Horizon servers share one world through a proxy and a fenced store. M1 and M3 are complete. Crossing, terrain and vehicle checks pass with Horizon's client. A stock-client check was not run, by [D11](docs/decisions.md#d11-horizons-client-stands-in-for-the-vanilla-client). Under the matched four-core server budget, the measured frontier is 650 players against 325 on plain Horizon, or 2.0 times the baseline. The 3x target is not met. Proxy-path P95 is 7.31 ms against a 5 ms target. The tick-cost bound is unproven, and the soak stopped on lease expiry at about minute 23. See [current progress](docs/progress.md), [E20's capacity result](docs/experiments/E20-frontier.md) and [E23's closeout checks](docs/experiments/E23-closeout.md).

[E28](docs/experiments/E28-linux.md) repeats the M4 measurements on a second machine, a 6-core Linux PC with the budget enforced by core pinning ([D12](docs/decisions.md#d12-second-platform-linux-with-core-pinning)). Its numbers stand beside the Mac's and replace none of them. There the plain baseline is 200 players and the cluster frontier 425, or 2.125 times its own baseline. Proxy-path P95 is under 5 ms at 425 players with pinning. The test of whether pinning removes the Mac's limiter tail was inconclusive, because the limiter never paused anything at that load. Speculative warming costs about 0.6% of the busiest tile's main-thread CPU, and the combined additions stay unproven. The 30-minute soak fails on that machine. Two bugs it exposed are fixed, and the cause of the store stalls behind the lease losses is still open. The crash campaign is in progress.

Start with the [experiment index](docs/experiments/README.md) and [glossary](docs/experiments/README.md#glossary). [Design decisions](docs/decisions.md) record the scope and pass rules.

## Hardware and scope

Every run used one machine at a time. These are single-machine loopback measurements, not multi-host results. E1 to E27 ran on the Mac. The [E2 machine record](docs/experiments/E2-real-transfer.md#commits-machine-and-resources) identifies an Apple M5 Pro with 18 CPUs and 48 GiB RAM, running macOS 26.5.2 on arm64. Capacity runs give the server fixture four aggregate cores and bots one separate core, as defined in [D9](docs/decisions.md#d9-capacity-layout-and-equal-cpu). On the Mac a SIGSTOP limiter holds each group to its average.

E28 ran on the second platform: an Intel Core i5-8400 with 6 cores, no SMT and 16 GiB RAM, running Linux 6.18 with cgroup v2. There D12 enforces the same budget by pinning. CPUs 0 to 3 run the servers, CPU 4 the bots, and CPU 5 the harness and everything else on the host. A pinned core budget and a limited average are not the same constraint, so each machine's frontier is compared only with its own baseline.

The measurement scripts read their counters from libproc and mach on macOS and from `/proc` on Linux. Counters with no Linux equivalent are reported as null. Profiling with `sample`, `xctrace` and `vmmap` is macOS only. The Linux port changed scripts the Mac results came from, and those have not been rerun on the Mac since. The standalone Rust model needs neither platform's counters.

## History

The first two experiments were reproduced independently. Their crossing timings describe the early stack, not the current result:

- **[E1](docs/experiments/E1-durability.md):** a deterministic simulator crashes tiles mid-handover across 1,000,005 scenarios. A tile-wide checkpoint before a fenced player commit kept item duplication at zero in the model.
- **[E2](docs/experiments/E2-real-transfer.md):** a scripted walking player moved between two real Horizon servers through [Meridian](https://github.com/Akirabane/Meridian). All 100 moves worked with inventories intact, in two runs. The player can't act for about 480 ms at P95, against a 150 ms target.

Later [E10 crossings](docs/experiments/E10-cluster.md) include the cut and fenced store commit. E20 and E23 report the selected stack's capacity and closeout results.

## Current experiment stacks

E1 runs from this checkout. The current cluster results are E20's capacity experiment and E23's crossing and closeout checks. Their commands require a prepared Minecraft installation and a specific patched Horizon/Meridian stack. The frozen local binaries are not included in this repository. E28 rebuilt the plain-09 baseline and the final stack from fresh upstream clones at these pins and patch orders, on Linux, and recorded every patch, source and binary hash in [its build records](docs/experiments/data/E28/build-manifest.json). That reconstruction has not been repeated on macOS.

The following inventory records source identities and patch order. It is not a complete build recipe. Patch paths are relative to `patches/`. See the [patch index](patches/README.md) for what each delta changes and whether it qualifies.

### Plain-09 baseline

Horizon is pinned to `fc2868b2b26d56ec76e17414909fe58a3b87bb92`. Apply these patches in order:

1. `e2-horizon-input-trace.patch`
2. `horizon-03-bots.patch`
3. `horizon-04-digest.patch`
4. `horizon-09-flat-light.patch`

This stack has no internal goliath crate dependency. The [successful plain build record](docs/experiments/data/E18/flat09-plain-build-3.json) records the pin and order. Its patch-09 digest predates the canonical patch packaging. The [both-stacks proof](docs/experiments/data/E18/flat09-both-stacks-proof.json) records the canonical patch and identical compiled flat-terrain files. [E17](docs/experiments/E17-m4.md#matched-d9-plain-baseline) gives the matched 325-player baseline and fixture.

### FINAL-09 tiles

Horizon uses the same `fc2868b2b26d56ec76e17414909fe58a3b87bb92` pin. Apply these patches in order:

1. `e2-horizon-input-trace.patch`
2. `horizon-01-tile-cut.patch`
3. `horizon-02-tile-ownership.patch`
4. `horizon-03-cluster.patch`
5. `horizon-05-m2.patch`
6. `horizon-06-leases.patch`
7. `horizon-07-observer-status.patch`
8. `horizon-03-bots.patch`
9. `horizon-04-digest.patch`, with `--exclude=crates/horizon-client/src/game/dedicated/status.rs`
10. `horizon-08-m4-measurement.patch`
11. `horizon-09-flat-light.patch`

The [tile build record](docs/experiments/data/E18/flat09-tiles-build-1.json) lists this exact order and option. It produced engine SHA-256 `b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d`. The [E17 prerequisite manifest](docs/experiments/data/E17/final-release-prereqs-v2.json) records internal-source revision `b4dc5af04b41a70c68d0e5d8608c4d7dd983b3ec` and per-file hashes for the model, store and workspace configuration. Those hashes, not today's working tree, identify the internal sources.

That historical revision is outside published history. Every internal-source file listed in the manifest also matches published commit `25507cdf0819a0a530e6cbb36087a107cb890580`, the M3 completion commit. Use that reachable revision and verify the manifest's file hashes when preparing these internal crates. This source check does not establish a reproduced binary build.

### Final tiles

The final tile stack is FINAL-09, then `horizon-13-lease-renewal-r2.patch`, then `horizon-10-warm-tick.patch`. It runs with the bounded store from this repository and the reactor-1 proxy below.

E28's `final-r2` tile adds `horizon-22-lease-read-deadline.patch` and `horizon-23-registration-refusal.patch`, in that order, for the two bugs its Linux soak exposed. It is built and gated on Linux only, and its soak has not passed.

### reactor-1 proxy

Meridian is pinned to `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`. Its Horizon protocol/world dependency is pinned to `0d21d5520ec26c49b774bf1533e53ea8af81278d`, as recorded in [E2](docs/experiments/E2-real-transfer.md#commits-machine-and-resources). Apply these patches in order:

1. `e2-meridian-client.patch`
2. `meridian-01-wall-trigger.patch`
3. `meridian-02-fast-crossing.patch`
4. `meridian-03-cluster.patch`
5. `meridian-04-m2.patch`
6. `meridian-05-proxy-cost.patch`
7. `meridian-06-io-pool.patch`

The [reactor build manifest](docs/experiments/data/E18/reactor-1-build.json) records the pin, ordered patch hashes and resulting source hashes. The internal model/store sources use the same `b4dc5af04b41a70c68d0e5d8608c4d7dd983b3ec` revision and file hashes in the [E17 prerequisite manifest](docs/experiments/data/E17/final-release-prereqs-v2.json). [E23's frozen identity table](docs/experiments/E23-closeout.md#frozen-identities-and-reproduction) identifies the proxy, tiles, store, sidecars and test clients used together. Later scheduler, warm-tick and shared-view candidates do not replace this accepted measurement stack.

### Inspect evidence or repeat a run

To inspect published evidence without running servers, read [E17's matched baseline](docs/experiments/E17-m4.md), [E20's staircase and reproduction rows](docs/experiments/E20-frontier.md), [E23's closeout tables](docs/experiments/E23-closeout.md), and [E28's second-platform tables](docs/experiments/E28-linux.md). Their `data/` records include raw counters, summaries and hash indexes. For example, `gzip -dc docs/experiments/data/E14/build10-full-off.json.gz | jq .` reads a compressed record without changing it. Check the [hash conventions](docs/experiments/README.md#evidence-conventions) before comparing digests.

Repeating a live run is a separate task. [E17's commands](docs/experiments/E17-m4.md), [E20's commands](docs/experiments/E20-frontier.md) and [E23's commands](docs/experiments/E23-closeout.md#frozen-identities-and-reproduction) assume their prepared resources, frozen artifacts and quiet-machine conditions. They do not create all prerequisites from a fresh clone.

## Run E1

E1 is standalone Rust, with no Horizon dependency or external crates. It needs
Rust 1.98 or newer. Build output stays in the root `target/`.

```sh
just test
just sweep                 # 1,000,005 scenario-seeds across 5 policies and 3 latency settings
just replay P1 6 0         # exact seed replay, then a deletion-minimised counterexample
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`just sweep` uses independent OS processes on all available cores. A scenario
itself has one scheduler, a seeded RNG and a virtual clock, with no threads or
wall clock. It writes `docs/experiments/data/E1-results.csv`. Latencies are
assumptions, not Horizon measurements.

For the same workload without injected faults:

```sh
cargo run --release -p goliath-sim -- calm 1000 docs/experiments/data/E1-calm.csv
```

The model crate owns tile images, authority, generation CAS, handover mutations,
checkpoints and delta replay. The simulator owns scheduling, fault delivery,
policy timing, replay reduction and sweep processes. See
[E1 durability results](docs/experiments/E1-durability.md) for the limits and recommendation.

## What it won't do yet

Only players cross a wall. Redstone, fluids, mobs and items stop there, as they reportedly do on DonutSMP. A shared edge band that lets them cross is a later phase, and no shipped Minecraft server has done that.

## Requirements

Horizon needs game files prepared from your own copy of Minecraft: Java Edition. This repo contains no Mojang code or assets and never will.

## Not affiliated

This project isn't affiliated with or endorsed by Mojang, Microsoft, DonutSMP, Horizon or Meridian.

## Licence

MIT. See [LICENSE](LICENSE).
