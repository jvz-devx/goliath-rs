# goliath-rs

An experiment in running one Minecraft world across many server processes, in Rust, on top of [Horizon](https://github.com/horizon-rust/horizon).

The world is one shared coordinate space split into static tiles. Each tile is a separate Horizon server. A player who walks through the wall between two tiles is handed to the neighbouring server without reconnecting. Vanilla Java Edition clients connect through a proxy and need no mods.

The design copies the publicly visible behaviour of DonutSMP's Goliath system. Its code isn't public, so nothing here comes from it.

## Status

Early and experimental. Nothing here runs a playable server yet.

The first two experiments are done, and both were reproduced independently:

- **[E1](docs/experiments/E1-durability.md):** a deterministic simulator crashes tiles mid-handover across 1,000,005 scenarios. A tile-wide checkpoint before a fenced player commit kept item duplication at zero in the model.
- **[E2](docs/experiments/E2-real-transfer.md):** a scripted walking player moved between two real Horizon servers through [Meridian](https://github.com/Akirabane/Meridian). All 100 moves worked with inventories intact, in two runs. The player can't act for about 480 ms at P95, against a 150 ms target.

Status is in [docs/progress.md](docs/progress.md), and decisions are in [docs/decisions.md](docs/decisions.md).

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
