# E2. Real Horizon transfers through Meridian

2026-11-07. Reproduced independently, see "Independent reproduction" below.

## Result

100 of 100 transfers completed between two real Horizon servers through real Meridian and two real
sidecars. Inventory checksums matched before and after every move. No loading-phase restart occurred.
The walking input gap P95 was 495.296 ms. The target of less than 150 ms was not met.

These are loopback protocol measurements with a scripted, kinematic player. They are not a video,
a human play test or a test of Minecraft client physics.

| Metric | Stationary P50 | P95 | P99 | Walking P50 | P95 | P99 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Total switch, ms | 699.643 | 749.682 | 749.685 | 949.328 | 999.730 | 1049.613 |
| Server-observed input gap, ms | 195.452 | 245.314 | 245.359 | 404.735 | 495.296 | 504.642 |
| Largest arrival correction, blocks | 0 | 0 | 0 | 1.40 | 1.68 | 1.96 |
| Clientbound TCP payload bytes | 12,484 | 12,825 | 12,846 | 10,585 | 14,156 | 15,166 |
| Horizontal server speed after arrival, blocks/tick | 0 | 0 | 0 | 0 | 0 | 0 |
| Absolute teleport delta speed, blocks/tick | 0 | 0 | 0 | 0 | 0 | 0 |

Each group has 50 samples. Percentiles use nearest rank, `ceil(p * n)`, without interpolation.
With 50 samples, P99 is the maximum. This is one run, not an estimate with confidence intervals.

Every trial had two absolute position packets, both with relative flags 0 and delta `[0, 0, 0]`.
After arrival the exported server velocity was `[0, -0.0784000015258789, 0]` in all 100 trials.
The vertical component is reported as observed, not rounded to zero.

No observed item loss or count increase. The known inventory was five diamonds and twelve oak logs,
17 items in total. Its SHA-256 was identical before and after all 100 moves. This checks the player's
inventory, not unique authority across stale save files, crashes, dropped items or containers.

## Commits, machine and resources

| Component | Exact revision |
| --- | --- |
| Horizon engine, bots, control tool and supervisor | `fc2868b2b26d56ec76e17414909fe58a3b87bb92` |
| Meridian proxy, sidecars, bench and testkit | `902dcf195e0a8bbd3f39a5f560684af00e64bf5d` |
| Meridian's Horizon protocol/world dependency | `0d21d5520ec26c49b774bf1533e53ea8af81278d` |
| Launcher release that prepared the resources | Nightly `2026-11-05-66f3981` |

Mac17,8, Apple M5 Pro, 18 physical/logical CPUs, 48 GiB RAM, macOS 26.5.2 build 25F84,
arm64. Rust 1.98.1. Python 3.15. No CPU affinity, exclusive-machine reservation or WAN emulation.

The launcher-prepared files were read directly from:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

The installed release and the built commit have equal `minecraft` sections in `research-lock.json`,
including the client and server SHA-1 pins and accepted manifests. The resource manifest names 26.3,
the facts and block-tags checksums match, registries and commands exist, and fonts/textures are nonempty.
The real servers also loaded them successfully. The launcher binary, accounts and settings were not used.
No jars were downloaded by this experiment. No prepared files were copied or linked into the repo.

The packaged v0.2.97 engine was not used. It lacks the transfer module. No newer Horizon commit was used.

## What ran

Two dedicated engines used separate fresh game directories, a flat world, seed 12345, survival mode,
view distance 3 and simulation distance 3. Both had transfers and Velocity forwarding enabled.
The same prepared resources supplied both. All game connections, sidecars and the proxy used loopback.
The proxy used offline authentication, `switch-carry-position = true` and one sidecar secret per backend.
No Microsoft account was involved. The player was a synthetic offline test identity.

The first 50 moves were stationary. The next 50 were straight-line movement along positive X at
5.6 blocks/s, nominally 20 position packets/s. This is sprint-speed kinematic movement, not a survival
walking physics simulation. Transfers alternated A to B and B to A. A command requested each transfer
while movement continued. There is no coordinate wall trigger, tile ownership or observer lane yet.

All runtime files lived below fresh `.local/runs/E2-*` directories. The runs set isolated
`HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, XDG homes and launcher scratch paths, disabled audio and
self-update, and gave the unchanged upstream harness a scratch `TMPDIR` there too.

### Timing definitions

- Total time starts when the scripted client sends `/server` and ends when it receives Meridian's
  successful arrival message. This includes client/proxy scheduling and confirmation messaging.
- Input gap is the timestamp of the first accepted target movement minus the timestamp of the last
  accepted source movement. The server trace is after validation and collision checks, where the
  authoritative position has been updated. Rejected packets and teleport acknowledgements are excluded.
  These are server-side observations, not Meridian's export-request or "freeze" timer.
- Accepted-movement timestamps use the same Mac wall clock across both processes. The trace also records
  server ticks and positions. The 50 ms input cadence is part of the last-input-to-first-input metric.
  It does not locate the freeze boundary to sub-tick precision. No telemetry overhead baseline was run.
- Arrival error is the largest Euclidean displacement from the client's current scripted position to
  an absolute arrival position during the switch. In the moving trials corrections were backwards
  along X, at most 1.96 blocks. This measures a protocol correction, not rendered camera displacement.
- Bytes are changes in the client's protocol connection receive counter between command and successful
  arrival message. They are compressed TCP payload bytes, including surrounding protocol traffic,
  not Ethernet/IP/TCP headers and not Meridian's uncompressed `delta_bytes`.
- Velocity comes from received teleport deltas and a server export after the first target input.
  The scripted client does not generate nonzero physical horizontal velocity. Therefore this run
  confirms zero arrival deltas, but does not establish how a real physics client loses momentum.

The moving group had 32 gaps above 250 ms and 18 below 250 ms. Its gaps ranged from 154.624 to
504.642 ms. The stationary range was 154.558 to 245.359 ms. None of the 100 gaps was below 150 ms.

## Commands and outcomes

Commands below are run from the repository root unless `cd` says otherwise. Every build used one
workspace-local `target/`, profile `fast`, and `CARGO_INCREMENTAL=0`. No examples, parity probes or
Rust test suite were built. No boat sandbox was used because the binaries had to run on macOS.

```sh
git clone --filter=blob:none --no-checkout https://github.com/horizon-rust/horizon.git .local/src/horizon
git -C .local/src/horizon checkout --detach fc2868b2b
git clone https://github.com/Akirabane/Meridian.git .local/src/meridian
git -C .local/src/meridian checkout --detach 902dcf1

cd .local/src/horizon
env -u CI CARGO_INCREMENTAL=0 cargo build --locked --profile fast --no-default-features \
  -p horizon-client --bin horizon-client -p horizon-bots --bin horizon-bots \
  -p horizon-client-lib --bin horizon-ctl -p horizon-server --bin horizon-server
```

The initial Horizon build passed in 203.20 s. Binaries were copied to `.local/bin/horizon-fc2868b2b/`.
The uninstrumented client is retained there as `horizon-client-baseline`.

```sh
cd .local/src/meridian
env -u CI CARGO_INCREMENTAL=0 CARGO_PROFILE_FAST_INHERITS=release \
  CARGO_PROFILE_FAST_LTO=false CARGO_PROFILE_FAST_CODEGEN_UNITS=16 \
  cargo build --locked --profile fast -p meridian-proxy --bin meridian \
  -p meridian-sidecar --bin meridian-sidecar -p meridian-bench --bin meridian-bench
```

Meridian passed in 71.00 s. Its binaries were copied to `.local/bin/meridian-902dcf1/`.
The environment defines its fast profile without modifying the upstream workspace manifest.

The unchanged upstream script was invoked with these arguments inside the isolated wrapper:

```sh
python3 .local/src/horizon/tools/server_transfer_e2e.py \
  --bin-dir "$PWD/.local/bin/horizon-fc2868b2b" \
  --resources "$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3" \
  --out "$SCRATCH/upstream"
```

`SCRATCH` denotes the wrapper's new temporary run directory, not a persistent directory.
The first run, with the uninstrumented engine, passed 37/37 checks in 15.954 s, exit 0.
The full-run wrapper repeated it with tracing disabled and passed 37/37 in 15.121 s, exit 0.
The script SHA-256 was `ec1d93186c65e0e94f026338c372d00b06f9709c6d554276687c2add4752931e` both times.
Its source is unchanged. It tests a transfer contract using two bot connections, not one seamless client.

The instrumented engine and testkit client needed one additional build each:

```sh
cd .local/src/horizon
env -u CI CARGO_INCREMENTAL=0 cargo build --locked --profile fast --no-default-features \
  -p horizon-client --bin horizon-client

cd .local/src/meridian
env -u CI CARGO_INCREMENTAL=0 CARGO_PROFILE_FAST_INHERITS=release \
  CARGO_PROFILE_FAST_LTO=false CARGO_PROFILE_FAST_CODEGEN_UNITS=16 \
  cargo build --profile fast -p meridian-testkit --bin e2-client
```

Both passed, in 154.95 s and 28.54 s respectively. The new binaries were copied out of `target/`.
These were builds of new instrumentation, not duplicate retries of an unchanged build.

```sh
python3 -B scripts/e2-real-transfer --skip-upstream --stationary 1 --walking 1
python3 -B scripts/e2-real-transfer
python3 -B scripts/e2-verify
scripts/clean-builds --all
```

The two-move pilot passed. The default run passed the upstream check and all 100 trials, exit 0.
The verifier independently recomputed all reported percentiles from retained trials, checked exact
gap arithmetic and correction distances, and checked every inventory checksum and both 37-check runs.

### Reproduce

With the retained binaries, the one-command experiment is:

```sh
python3 -B scripts/e2-real-transfer
```

For a checkout without the local sources or binaries:

```sh
python3 -B scripts/e2-real-transfer --build
```

`--build` clones the pins if absent, applies the saved patches, builds once per workspace, retains the
binaries, runs the experiment and invokes build cleanup. It never prepares or downloads game files.
Use `--resources PATH` for another already prepared 26.3 folder. The build helper was added after the
measured run; its syntax and patches were checked, but a fresh end-to-end `--build` run was not repeated.
The experiment itself and the smaller pilot were run using the retained binaries.

## Raw evidence and disk

Small retained evidence is in [data/E2](data/E2/):

- `trials.json`: all 100 trial measurements, source/target tick and position samples, corrections,
  velocity, byte counts, inventory checksums and summary statistics.
- `prerequisites.json`: exact pins, binary hashes, resource preflight, the second unchanged upstream
  check, its per-check outcomes and the completed run summary.
- `upstream-baseline.json`: the first unchanged upstream check with uninstrumented binaries.
- `pilot.json` and `pilot-prerequisites.json`: the successful two-move pilot, separate from the 100 samples.
- Four build `.log` and `.time` pairs: compiler outcomes and `/usr/bin/time -l` output.
- `cleanup.txt`: the observed disk high water and cleanup output.

The data directory is about 268 KiB before this report's small cleanup record. No world, game log,
player record, resource contents, real account name or real account UUID is retained there.

The largest observed E2 `.local/` sample was 3,029 MiB. Target directories reached 1,965 MiB for
Horizon and 435 MiB for Meridian. These are sampled high-water values, not continuous disk monitoring;
short-lived compiler files may have raised the true peak. The full transfer run used 6 MiB before
its runtime directory was deleted. Binaries retained outside targets occupy 197 MiB.

`scripts/clean-builds --all` deleted both upstream targets and an idle 95 MiB root target, as requested.
It freed 2,495 MiB and reported 621 MiB for the project afterwards. The root target belonged to shared
build output, not source. No `crates/`, root `Cargo.toml` or E1 source/results were changed by E2.
All experiment engines, clients, proxies and sidecars were stopped. `.local/runs/` is empty.
E2 made no commit, push, GitHub issue, comment or PR. Publication is separate.

## Research compared with observation

| Research expectation | Observation | Interpretation |
| --- | --- | --- |
| About 1.7 s total | P50 0.700 s stationary, 0.949 s moving | Not reproduced on this local, smaller-view setup. It does not invalidate the author's WAN run. |
| Inputs unavailable for 400 to 580 ms | Moving P50 405 ms, P95 495 ms; stationary P95 245 ms | Supported for the slower moving trials, not a universal baseline. |
| Velocity dropped | Every arrival packet carried zero delta | Zero arrival delta confirmed. Actual physical momentum loss remains unmeasured. |
| Snap-back 2 to 3 blocks | Moving P95 1.68 blocks, maximum 1.96 | Backward correction confirmed, expected size not reached here. |
| About 17 MB per move | P95 12,825 bytes stationary, 14,156 moving | Not comparable. Matching flat terrain and view 3 make the delta small, and our byte count is compressed. |
| Meridian's old protocol pin may not work with current Horizon | 100/100 real switches succeeded | The specified pins interoperate on this exercised path. Not a general packet or version compatibility claim. |
| The stock Horizon transfer check may be broken | 37/37 checks passed twice | No defect reproduced in that script at this pin. |

## Defects and reproductions

### Input gap exceeds the crossing target

Measured. Run the default experiment and recompute `input_gap_ms` from `first_target_input.ns` minus
`last_source_input.ns`. Stationary P95 is 245.314 ms, walking P95 is 495.296 ms. Both exceed 150 ms.
The scripted client continues to send positions during the switch. The server trace contains no
accepted movements between those two timestamps.

Source explanation, not separately instrumented phase attribution: Meridian's
`crates/meridian-handoff/src/switch.rs:183` sets a 200 ms settle time.
Lines 953 to 965 wait for a fresh post-import chunk batch to settle when the view recenters.
This work is inside the frozen window. `session.rs:1038` to `:1075` continues writing client packets
to the active source lane rather than maintaining an input replay queue. Tick-bound control requests
and placement also cost time. This run did not label each individual dropped packet or phase duration.

### The client is corrected twice during a moving transfer

Measured. The default walking trials each retain two entries in `arrival_teleports`. Both are absolute,
with zero velocity delta. Compare each entry's `before` and `pos`: the largest backwards correction
was 1.96 blocks and P95 was 1.68. The first pilot's moving trial corrected by 0.56 blocks on the second
position packet. This is directly reproducible with the two-move command above.

The source carries a snapshot pose without velocity in Meridian `switch.rs:466` to `:478`.
Its delta emits an absolute position in `crates/meridian-ghost/src/delta.rs:62` to `:73`.
Horizon also sends the placement confirmation. The received packet pair is measured; a visible real-client
stutter and physical deceleration are inferences that need a physics client or video.

### Experiment setup error, fixed before measurement

The first attempted Meridian pilot used one secret file for both sidecars. `meridian --check` correctly
rejected it before any client joined. The runner now creates separate secret files. This was a runner
configuration error, not an upstream defect, and contributes no samples to the tables.

No upstream functional fix was applied. Only the server trace and test client patches were needed.
No upstream transfer-script defect, protocol incompatibility or inventory mismatch was reproduced.

## Smallest next changes for a prewarmed crossing

These are source-based proposals, not measured fixes or a promise of P95 under 150 ms.

1. Split Meridian prepare from activation. Start it on approach, with a predicted crossing position.
   Horizon must accept that position before sending the held player's view, rather than preparing at
   a saved position and recentering after freeze. Remove the 200 ms post-import settle wait from the
   crossing by proving readiness before the source freezes. A preparation handle must expire or abort safely.
2. Shorten the frozen commit sequence. Prewarming alone cannot fix the stationary P95 of 245 ms.
   Batch target installation and durable activation into one server-tick operation and deliver the first
   valid queued movement in the activation tick. Evaluate Horizon's existing `player-import` placement
   mode before inventing another API. Keep ownership checks and durable commit; do not just skip confirm.
3. Add sequenced input buffering and replay across the lane change. Carry velocity, and remove or replace
   the second stale absolute teleport so replayed movement is not overwritten by placement confirmation.
   Test with a real physics client as well as this kinematic probe.

For D3's visible terrain beyond the wall, Horizon also needs the observer/positioned-view mode and
Meridian needs an owner-aware read-only chunk composition path. That was not implemented or measured here.
Fenced shared inventory ownership and crash recovery are separate requirements. Healthy checksum equality
does not establish them. Rerun at least 100 moves after each proposed crossing change before claiming
the 150 ms target.

## Remaining limits

No runtime prerequisite is blocked.
The fresh `--build` path, physical movement continuity, attacks/use during handover, WAN latency,
larger view distances, different terrain, crash safety, automatic wall triggering, terrain composition
and loaded-server behaviour remain untested. No rendered player-feeling number is claimed.

## Independent reproduction

The reviewer reran the full experiment on 2026-11-07 with `python3 -B scripts/e2-real-transfer`, using the retained binaries, and recomputed every number below from the raw trials with separate code. It also read the input trace patch: the timestamp is taken after the server accepts a movement, not from a proxy timer.

| Metric | First run P50 / P95 / P99 | Reproduction P50 / P95 / P99 |
| --- | ---: | ---: |
| Stationary input gap, ms | 195.5 / 245.3 / 245.4 | 223.2 / 226.8 / 226.8 |
| Walking input gap, ms | 404.7 / 495.3 / 504.6 | 423.2 / 476.8 / 476.9 |
| Stationary total, ms | 700 / 750 / 750 | 700 / 750 / 750 |
| Walking total, ms | 949 / 1,000 / 1,050 | 900 / 1,000 / 1,050 |
| Walking arrival correction, blocks | 1.40 / 1.68 / 1.96 | 1.40 / 1.68 / 1.68 |

Both runs completed 100 of 100 moves with every inventory checksum equal. Neither had a single gap under 150 ms: the lowest were 154.6 and 173.1 ms. Values move in steps of about 50 ms, one server tick, so the difference between runs is the phase of the tick and not a change in behaviour. Each run is one sample of that phase, so treat the percentiles as a range. Raw data is in `data/E2/repro-trials.json`.

One correction: the table at the top reports 0 horizontal speed after arrival for all trials. That holds for the first run. One walking trial in the reproduction reported 0.25 blocks per tick, so zero is typical and not guaranteed.

Build logs and JSON evidence had absolute paths replaced with `$REPO` and `$HOME` before publishing, and the runner now writes `$HOME` itself. The fresh `--build` path is still unverified.
