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
