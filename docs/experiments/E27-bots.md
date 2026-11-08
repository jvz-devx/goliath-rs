# E27. Per-bot route and backend-switch audit

Status: passed in the corrected builder cell. All 40 bots complete the route
without disconnecting or reconnecting. All 435 measured switches have proxy
and tile confirmation. The live connection-kill control fails as required.
The reviewer must reproduce this before accepting the M4 item. No milestone
box is ticked. The failed first fixture remains in the evidence.

## Question and pass line

Can the frozen `horizon-bots` follow a set route through a backend switch
without reconnecting?

Every route bot must complete one ordered cycle, keep one front session and
have zero reconnects. Every bot must switch at least once. Every proxy
activation must have a matching committed target, a completed proxy move,
target-side placed state, source-side released or absent state, and subsequent
movement reaching the target backend. A target must also observe at least
0.4 blocks of further movement before the next switch.

The live failure control must fail this same audit. It uses a separate bot on
the same path. The wrapper kills its front connection 12 seconds after launch.

This is a correctness cell. It is not a capacity or latency measurement.
There are no changes to bot, proxy, tile, store or sidecar binaries.

## What the retained data proves

`scripts/e27-audit history` reads the retained top-level E20 and E23 JSON
records. The inventory contains 18 D9 cell rows and nine E23 tap records.
It records decompressed hashes for every inspected raw record.

Seventeen completed D9 rows have proxy activations for every crossing bot.
For example, E20's frozen reactor-1 650-player first repeat has 587 activations
across all 65 crossing bots, with 8 to 10 per bot in the measured interval.
E23's latency control has 587; its witness cell has 585. Both cover all 65.
The failed E23 soak lacks this complete measured activation ledger.

The D9 route and bot commands are retained. The records also contain named
proxy move lines, UUIDs, and tile positions at join, before and after the
measured interval. These prove real handovers and sampled tile positions.
The aggregate bot rows report online counts, errors and Transfer reconnects.
Together with the frozen client code, they support continuity, but they are
not a retained per-session connection ledger.

E23's taps retain trajectories for eight preselected witnesses. Only
`E17BotX62` and `E17BotX63` are crossing bots. Their front tap contains 2,288
and 2,282 positions across the full fixture lifetime. All other crossing bots
lack this position history. The tap records omit connection identity and
connection-open/close events. E20's D9 rows have no retained all-bot wire trace.

The native physics and vehicle checks use other clients. They do not audit
`horizon-bots`. Local E20 and E23 evidence contains native profiles, physics
images and availability traces, not the missing all-bot session ledger.

The old data cannot establish an ordered full route for each bot, maximum
deviation around each switch, or per-switch placed/released confirmation on
the tile side. A dedicated cell is needed.

Inventory: `data/E27/e27-retained-v2.json`. The earlier inventory remains as
`e27-retained-v1.json`; v2 adds stack identities to distinguish candidates.

## Fixed plan

Use four 2,048-block tiles, D9's tile indices and its exact crossing path:

```text
(-12,12) -> (12,12) -> (12,-12) -> (-12,-12)
         -> (12,12) -> (-12,12) -> (-12,-12) -> (12,-12)
         -> (-12,12)
```

The two diagonal legs cross the corner. Forty bots share this cyclic path.
Speed is 4 blocks/s, height offset 2, view distance 3 and join cadence 0.1 s.
The audit window is the first 75 seconds after launch. The driver runs for
90 seconds, leaving 15 seconds to confirm switches near the audit boundary.
This split is fixed before the cell. Every bot must finish by second 75.
Switch counts cover activations in those 75 seconds. Later activations remain
in the raw file and have a separate per-bot count. Connection continuity is
checked over the whole 90-second driver lifetime. Its initial target is bot index modulo four,
as in the frozen client. The audit records that phase for each bot.

Each tile gets a local spawn near the route. Login uses a forced host to tile
b. The first leg acquires the initial waypoint from the observed spawn.
Completion means reaching that waypoint, then all eight subsequent waypoints
in order. The driver continues after this finite audit route completes.

The wrapper adds forwarding taps at the front and all four backends. They
decode all E27 bots, not a sample. They retain connection identities, login,
Play entry, every movement payload hash and position, and connection closure.
Only the handshake host changes. Movement payloads are not rewritten.

The controller samples each tile's actual player state every 0.1 seconds plus
query time. It keeps query send/receive brackets and excludes terrain readers.
Placed target state must be unheld, unfrozen and unreleased. Proxy activation
records count switches; the client's configuration counter does not.

The audit reports maximum horizontal distance from the current ordered segment
before and after every activation. It separately reports actual placed tile
positions' distance from the D9 polyline. The initial acquisition leg is
explicit, not mistaken for a completed path. The ordered client tolerance is
0.000001 blocks. The tile polyline limit is 0.75 blocks, including centred
spawn placement. Vertical positions remain in the raw file.

Both taps add TCP hops. Their timing is not proxy-only latency and is not
evidence for E25's later direct-path latency work. No handshake-host option
or packet-send stamp was added to the frozen client.

The cell has no interior population, pigs or CPU limiter. It cannot replace
D9 capacity cells. It uses only the D9 geometry and crossing route. Whole-host
CPU rows redact outside names to `outside` and retain PID/counter evidence.

## Commands

E24's soak has priority. Hold the window only for the cell and its cleanup.
Use a fresh result name for a reproduction.

```sh
python3 -B scripts/e27-tests
python3 -B scripts/e27-audit history --name repro-e27-retained
.local/bin/host-window quiet E27 "per-bot route/session audit after E24"
python3 -B scripts/e27-cell --name repro-e27-route
.local/bin/host-window release E27
python3 -B scripts/e27-audit cell --raw docs/experiments/data/E27/repro-e27-route.json.gz --name repro-e27-summary
scripts/clean-builds
```

Release the window on failure too. The measured invocation uses a shell EXIT
trap for this. The runner records failed attempts and removes its private
world and log directory after extracting evidence.

## Verification

Eight resource-free E27 checks pass. They cover a complete ordered route with
both-side confirmation, a reconnect despite route completion, a killed
connection with an unfinished route, a missing target placement, wrong or
stationary movement, fragmented/compressed position decoding, timestamp-free
CAS pairing and relative evidence paths. Twelve unchanged E18 measurement-tool
tests and seventeen unchanged E23 checks also pass. `git diff --check` passes.
No Rust code was changed or built. No Rust gate is claimed.

## Results and hashes

The first cell, `e27-route-r1`, used height offset 0 instead of D9's existing
offset 2. It retained 40 online bots, zero reconnects, and 70 confirmed switches,
but zero completed routes. Ten bots never switched. The route checker rejected
all 40. Aggregate online counts would have missed this failure.

The raw data records 59,767 corrections. Thirty bots end near
`(-8.323,-60,-8.323)` on the diagonal towards tile c's marker. The other ten
end near `(-8.316,-60,9.180)` on their initial diagonal near tile b's marker.
The launcher places gold markers at `(-8,-60,-8)` and `(-8,-60,8)`.
Height offset 0 leaves feet at ground level. D9's offset 2 clears these markers.

The repair changes only the wrapper's `--height` argument to the existing D9
value 2. It changes no frozen executable, route, pass line or audit duration.
The repaired attempt is `e27-route-r2`. The failed raw cell and its summary
remain intact. No successful first-cell route is claimed.

Two checker serialization defects were fixed without another load run.
Frozen CAS rows lack timestamps, so each activation now pairs with the
preceding same-UUID CAS in original log order. Evidence paths now resolve
before repository-relative identity recording. Both failures are retained in
`e27-analysis-v1-error.json` and `e27-analysis-v2-error.json`. Regression tests
cover both cases. The first complete derived result is `e27-summary-v3.json`.

### Corrected cell

The corrected cell completes in 94.365 seconds including startup, extraction
and cleanup. All 40 bots finish their ordered cycle between 53.651 and 65.702
seconds after launch, before the fixed 75-second boundary. Every bot keeps
exactly one front connection throughout the 90-second driver lifetime.
There are zero disconnects, reconnects, native errors, corrections and late
movement rounds. The native configuration-switch counter remains zero, as
expected for seamless handovers. It is not the backend-switch count.

All rows below use route R, the exact eight-waypoint D9 path above. The names
are `E27BotX0` through `E27BotX39`. Initial target means index modulo four.
The complete per-bot table and every switch's evidence are in
`data/E27/e27-summary-v4.json.gz`. No bot is sampled or omitted.

| Initial target | Bots | Measured switches | Switches per bot | Finished | Reconnects |
| --- | ---: | ---: | ---: | ---: | ---: |
| R0 | 10 | 110 | 11 | 10 | 0 |
| R1 | 10 | 110 | 11 | 10 | 0 |
| R2 | 10 | 106 | 10 to 11 | 10 | 0 |
| R3 | 10 | 109 | 10 to 11 | 10 | 0 |
| Total | 40 | 435 | 10 to 11 | 40 | 0 |

All 435 activations pair with one preceding same-UUID CAS and one completed
proxy move. Each has subsequent target-side placed state and source-side
released or absent state. Each target receives unchanged movement payloads
seen at the front. All 435 also show further target-side position progress,
at least 11.475 blocks before the next switch. Corner handovers are present,
including c to a and d to b.

The raw summary gives maxima before and after every individual switch.
The intervals run between adjacent activations, with the initial join leg
included before the first activation. Here are the largest values across bots.
These are horizontal distances because the `--path` argument defines X/Z.
All three coordinates remain in the raw positions.

| Position evidence | Before a switch, blocks | After a switch, blocks |
| --- | ---: | ---: |
| Front movement, current ordered segment | below 0.000001 | below 0.000001 |
| Actually placed tile positions, D9 polyline | 0.669615 | 0.328023 |

The ordered front maximum is `1.7245221725840607e-14` blocks, floating-point
roundoff. The larger tile value includes centred initial spawn placement.
Every sampled placed position remains below the fixed 0.75-block limit.

The full 90-second driver lifetime has 524 activations for the 40 route bots.
All 524 have proxy/source/target/movement confirmation. The extra 89 belong
to the confirmation tail, outside the fixed pass window. A supplemental full
ledger is in `data/E27/e27-full-driver-v1.json.gz`. Its last switch for
`E27BotX4` has only one target position and one backend movement packet before
normal driver exit. Further 0.4-block progress for that tail switch is not
proven. It is not counted among the 435 qualifying switches. The fixed window
and its tail were selected before either cell ran.

`E27Control0` switches twice, then the wrapper kills its front connection at
about 12.1 seconds. Its native report has a connection-reset error. The audit
records one disconnect, no reconnect and no finished route. It rejects the
control with the same checks used for the other bots.

The corrected cell starts at one-minute load 3.490, below 4. Its largest
outside process averages 0.069884 core, below 0.5. Frozen hashes match before
and after both cells. Both holds last about 94 seconds, excluding the
scheduler's build-drain wait. Each releases immediately after fixture cleanup.
No E27-owned world, log directory, process or host-window hold remains.

Plain `scripts/clean-builds` freed 0 MB. It skipped the shared Horizon target
while another build used it. E27 owns no source checkout and changed no
Horizon, bot or Meridian source. No commit, push or upstream action occurred.

Verdict: the corrected frozen-stack cell proves the M4 bot behaviour in this
finite D9 route test. It does not prove stock-client behaviour, capacity,
proxy-only latency or long-run survival. Acceptance awaits reviewer reproduction.

### Stored data

Every SHA-256 below covers the decompressed bytes. Large JSON files use
lossless gzip. The integrity record verifies compression, redaction, filenames,
frozen identities, source hashes and resource-free checks.

| File under `data/E27/` | SHA-256 |
| --- | --- |
| `e27-retained-v2.json` | `31b107998150f512df7c51869e686352ca285b8fe3f05ce9c67854d93b242f68` |
| `e27-route-r1.json.gz`, failed fixture | `6e47ac2d506d66e0cb9df64c15706e2506acc526434641f99a121e20787b6a0b` |
| `e27-summary-v3.json`, failed fixture audit | `de530a811c196d7b12b0895ec9e1a6f15c2180fa7ce54732fd7c0f9ef638f056` |
| `e27-route-r2.json.gz`, corrected cell | `c91853e5eaca7244879da232a335973c1cf3f32d0728db39a79895256d8e117d` |
| `e27-summary-v4.json.gz`, per-bot audit | `b59d62b50bf421dfd72e608a4a56643fac12dd845da6ffc483a7e65bb6313e67` |
| `e27-full-driver-v1.json.gz` | `f9dc6b46e17a488d88663f5f4dde542c4e3caa79ec5998e48ff934a48b4c80da` |
| `e27-integrity-v1.json` | `b471c50f9901fe4e2763c2bb03a6007f9f7f1d08f1adefca348d7361f64c04d0` |

The first inventory and both failed checker-command records are also retained.
Their hashes are in `e27-integrity-v1.json`. The corrected raw JSON is
62,828,510 bytes before compression. No raw rows were trimmed.

The frozen bot is `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d`.
The proxy is `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b`.
The FINAL-09 tile is `b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d`.
Raw cells and the integrity record include all six executable identities.

Executed load names were `e27-route-r1` and `e27-route-r2`. The corrected
analysis command was:

```sh
python3 -B scripts/e27-audit cell --raw docs/experiments/data/E27/e27-route-r2.json.gz --name e27-summary-v4
python3 -B scripts/e27-tail-audit --raw docs/experiments/data/E27/e27-route-r2.json.gz --name e27-full-driver-v1
python3 -B scripts/e27-integrity --name e27-integrity-v1
```

These result names now exist. Use fresh `repro-` names to repeat the commands.
