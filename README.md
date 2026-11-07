# goliath-rs

An experiment in running one Minecraft world across many server processes, in Rust, on top of [Horizon](https://github.com/horizon-rust/horizon).

The world is one shared coordinate space split into static tiles. Each tile is a separate Horizon server. A player who walks through the wall between two tiles is handed to the neighbouring server without reconnecting. Vanilla Java Edition clients connect through a proxy and need no mods.

The design copies the publicly visible behaviour of DonutSMP's Goliath system. Its code isn't public, so nothing here comes from it.

## Status

Early and experimental. Nothing here runs a playable server yet.

Two experiments come first:

- **E1:** a deterministic simulator that crashes tiles mid-handover, to find which commit and recovery policy keeps item duplication at zero.
- **E2:** the first real measurements of moving a walking player between two Horizon servers through [Meridian](https://github.com/Akirabane/Meridian).

Decisions and open questions are in [docs/decisions.md](docs/decisions.md).

## What it won't do yet

Only players cross a wall. Redstone, fluids, mobs and items stop there, as they reportedly do on DonutSMP. A shared edge band that lets them cross is a later phase, and no shipped Minecraft server has done that.

## Requirements

Horizon needs game files prepared from your own copy of Minecraft: Java Edition. This repo contains no Mojang code or assets and never will.

## Not affiliated

This project isn't affiliated with or endorsed by Mojang, Microsoft, DonutSMP, Horizon or Meridian.

## Licence

MIT. See [LICENSE](LICENSE).
