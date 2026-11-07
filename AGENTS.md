# goliath-rs

A sharded Minecraft world on Horizon: static tiles, one server process each, players handed over at a wall. Current status and milestones are in `docs/progress.md`; keep it up to date. Decisions are in `docs/decisions.md`, experiment results in `docs/experiments/`. Research notes in `docs/research/` are local and not published.

## Who does what

One agent researches, orchestrates and verifies. Another builds. Results count only after the reviewer reproduces them.

## Disk rules

Build output is disposable, and it must never pile up.

- One target dir per workspace. No extra `CARGO_TARGET_DIR` and no copies per experiment. At most two Horizon checkouts exist, `.local/src/horizon` and `.local/src/horizon-tiles`, each with one owner at a time.
- Finished binaries you want to keep go to `.local/bin/<project>-<commit>/`. Everything under a `target/` can be deleted at any time.
- Build Horizon and Meridian with `CARGO_INCREMENTAL=0`, in one profile only.
- Run `scripts/clean-builds` when a task ends. `scripts/clean-builds --all` deletes every target dir not in use, and is right once an experiment is finished.
- A run's world and log folders live under `.local/runs/` and are deleted after the run. Keep only small results, under `docs/experiments/data/`.
- Never run a second cargo build of the same workspace at the same time.

## Rust rules

Follow the `rust-project-architecture` skill.

- Our crates are application-internal until a release decides otherwise, so skip the library API rules.
- Workspace lints come from that skill's `assets/workspace-lints.toml` and `assets/clippy.toml`, and every crate inherits them with `[lints] workspace = true`.
- Add a crate only for a real boundary. `goliath-model` runs in production and `goliath-sim` doesn't, which is that kind of boundary. A tidy tree is not.
- No `types.rs`, `utils.rs` or `common.rs`. Newtypes for ids and epochs. Traits only for a real seam. Every crate and module opens with a `//!` line saying what it owns.
- Library code owns no threads or runtime.
- These rules cover our crates only. Code under `.local/src/` follows its own project's rules, and any patch to it stays small and lives in `patches/`.

## Gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Licence

Don't read Folia, Paper or other GPL source, or `docs/research/05-folia.md`, when writing code. Horizon's CONTRIBUTING bars it for anything sent upstream. Nothing goes to GitHub unless the maintainer asks.
