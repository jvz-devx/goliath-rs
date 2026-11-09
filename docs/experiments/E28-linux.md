# E28. Second platform: Linux with core pinning

Status: baseline and cluster frontier reproduced by the reviewer. Latency,
tick-cost and soak stages are in progress and not claimed here.

## Question

Do the M4 results hold on a second machine, with the CPU budget enforced by core pinning instead of the SIGSTOP limiter? What is this machine's frontier-to-baseline ratio?

E28 is a second platform, not a correction. Every Mac result stays as it is. Numbers here are compared only with this machine's own baseline.

## Machine

| Item | Value |
| --- | --- |
| CPU | Intel Core i5-8400, 6 cores, no SMT, 2.8 GHz base |
| Memory | 16 GiB, 8 GiB swap |
| OS | NixOS, Linux 6.18, cgroup v2 with the cpuset controller |
| Toolchain | rustc 1.99.1 |

## CPU budget

[D12](../decisions.md#d12-second-platform-linux-with-core-pinning) keeps D9's four server cores and one bot core, and enforces them with cpusets:

| CPUs | Runs |
| --- | --- |
| 0 to 3 | tiles, proxy, store, sidecars; or the one plain Horizon process |
| 4 | bots |
| 5 | harness, test-only front shims, and every other process on the host |

The SIGSTOP limiter stays as a comparison mode (`limiter`). The default mode is `pin`.

## Plan

Work runs in this order. Each step names its owner. Builders get exclusive files and checkouts.

1. **Harness port** (builder H). Add a platform layer to `scripts/`: `/proc`-based process and thread counters in place of libproc and mach, a `GOLIATH_RESOURCES` override for the resources path, the D12 cpuset layout, the system-slice narrowing and restore, frequency and throttle recording, and a `--cpu-mode pin|limiter` switch. Counters with no Linux equivalent are reported as null, never faked. Mac behaviour stays unchanged. The Python harness tests pass on Linux.
2. **Fresh rebuild** (builder B). Clone Horizon and Meridian from upstream at the README pins into `.local/src/{horizon,horizon-tiles,meridian}`, apply `patches/` in the documented order, and build release binaries with `CARGO_INCREMENTAL=0`, one target dir per workspace. Binaries: plain-09 server and bots; final tiles (FINAL-09, `horizon-13-lease-renewal-r2`, patch 10); the reactor-1 proxy, sidecar and e4 client; the bounded store from this repository; the lite variant (final tiles plus `horizon-21-lite-cpu-clean`); the latency-stamp tiles and bots (`horizon-15-...-lease-r2-b2`). Each is frozen under `.local/bin/goliath-e28/` with a build record of pins, patch hashes, gates and binary hashes. The Linux BOOTTIME path of the lease keeper is checked by build, lease tests and a short run. This is the first fresh-clone reconstruction of the stack.
3. **Plain-09 baseline staircase** under `pin`. Frontier rule: the highest step with two passes, closed by two fails one step above.
4. **Cluster frontier** on the final stack under `pin`, same rule.
5. **Proxy latency** with E25's no-extra-hop timestamps, at this machine's frontier, under `pin` and under `limiter` at the same load. The comparison tests D12's reason.
6. **Tick-cost lite rows** on the final stack, as in E26b.
7. **30-minute soak** at this machine's frontier.
8. **M3 campaign**, optional: 1,000 crash seeds, 12 partition and 6 stale-login cases.

Steps 1 and 2 run in parallel. Measurements run one at a time inside host windows.

## Results

The matched plain-09 baseline is 200 players: two passes at 200 and two valid
failures at 225. The direct-routing harness-v3 cluster frontier is the highest
stable tested pass, 425: two passes at 425, a mixed 450 step, and two valid
raw-P99 failures at 475. The tested upper bracket is 475, not an exact maximum.

The platform-local frontier/baseline ratio is **425 / 200 = 2.125x**. Only v3
qualifies it. V1, v2, pilots, smokes and diagnostics remain in the full ledger
but are excluded from the final frontier. The reviewer reproduced both brackets (below).

There are 26 retained cells: 25 valid, including six valid performance failures,
and one invalid startup. Nineteen cells pass. No mixed step got an extra repeat.
Every complete raw row over 1 MB is losslessly compressed. The 26 rows contain
100,281,041 decompressed bytes stored in 8,392,150 bytes, with the startup's
11,339-byte native failure evidence retained separately in 1,576 bytes.


### Reviewer reproduction

The reviewer ran every baseline and v3 frontier step again, two fresh repeats each, with the builder's documented commands, the same bundles and `--cpu-mode pin`. All 12 cells are valid. Data: `data/E28/reviewer-baseline/` and `data/E28/reviewer-frontier/`.

| Step | Repeat 0 | Repeat 1 | Server cores | Proxy cores |
| --- | --- | --- | --- | --- |
| Plain 200 | pass, 19.99 TPS | pass, 19.99 TPS | | |
| Plain 225 | fail, 19.67 TPS | fail, 19.57 TPS | | |
| Cluster 400 | pass | pass | 2.46, 2.41 | 0.64, 0.65 |
| Cluster 425 | pass | pass | 2.64, 2.67 | 0.69, 0.72 |
| Cluster 450 | fail | fail | 2.97, 2.94 | 0.79, 0.79 |
| Cluster 475 | fail | fail | 3.29, 3.30 | 0.87, 0.88 |

The baseline reproduces at 200 and the frontier at 425, so the ratio is 2.125x on this machine. The reviewer's 450 step failed both repeats where the builder's was mixed, which closes the bracket one step lower. The cluster fails on raw tick P99 while the four tiles together use under 3.3 of the four server cores: on this CPU each tile's main thread runs out of room first, not the core budget. The Mac's ratio was 2.0x (650 / 325).

## Harness port

The following describes the port handoff before game-server measurements.
Those builder port checks are separate from the capacity results below.
Darwin's libproc and mach counter paths remain in place.

`scripts/goliath_platform.py` reads process and thread counters from `/proc`.
Thread totals use nanosecond schedstat counters. Process totals retain observed
retired threads and the process stat lower bound; threads that exit between
samples have only clock-tick precision. User/system splits use stat clock ticks.
Main-thread identification uses TID equal to PID, independent of the truncated
Linux thread name. Context-switch counters cover live threads and say so.
Unix/Mach syscall counts, Mach page-ins and copy-on-write counts are null and
labelled unavailable. Linux TCP sampling uses socket inodes and `/proc/net/tcp`.
`sample`, `vmmap` and xctrace profiling reject Linux with a macOS-only message.

The runners accept `--cpu-mode pin|limiter`. Linux defaults to `pin`; Darwin
keeps the limiter. Before exec, a single-threaded wrapper joins the server,
bot or harness cgroup. Private cluster environments preserve that placement.
The cell narrows the existing top-level system slices and `init.scope` to CPU 5,
sets the governor to performance, leaves turbo alone, and journals the original
settings before changing them. SIGINT, SIGTERM and SIGHUP unwind cleanup.
`python3 -B scripts/goliath-cpu restore` retries an interrupted restoration and
is safe to run again. After SIGKILL, run it before removing any stale host lock
or starting another measurement.

Each Linux process report includes per-CPU busy ticks, per-core frequency
samples, throttle counters, available memory, swap-in pages, fixture placement,
outside CPU per PID and its total, and the CPU mode. Outside names are `outside`.
Start load above 4, outside work above 0.5 core per process, CPU 5 at least 95%
busy, missing throttle coverage or a throttle increase invalidate a cell.

`GOLIATH_RESOURCES` overrides the resources directory. The Linux default is
`$HOME/.local/share/goliath-resources/26.3`. Resource files stay outside Git.
The Linux identity anchor is `data/E28/build-manifest.json`. It hashes every
file in each E28 bundle, including metadata, launcher and marker, and is checked
before and after a cell. Executable, launcher and snapshot cross-checks remain
enabled. The builder's local bundle-layout specification is
`$REPO/.local/evidence/E28/bundle-layout.md`.

The baseline accepts `--bins` and an E28 `--data-dir`; its Linux staircase can
search below 300 players. Frontier, latency and lite cells also accept a data
directory. Latency and soak accept this machine's player count. `e20-results`
requires an explicit `--baseline` for Linux rows. Lite readback requires four
same-platform `--controls` and a fresh `--calibration`; it cannot silently reuse
Mac data. Finalized raw JSON above 1 MB is compressed, with decompressed-byte
hashes retained.

Builder verification used these Python suites:

```sh
mkdir -p .local/runs .local/evidence/E26
for suite in e14-runner-test e17-tools-test e17-d9-tests e18-tools-test \
  e20-tools-test e23-tests e23-soak-contracts e24-tests e25-tools-test \
  e26-tests e26b-tests m3-chaos-test e28-platform-tests; do
  python3 -B scripts/$suite
done
```

These suites pass. Seven tests are explicitly skipped on Linux because their
retained Mac executables or private Git indexes are absent. The new platform
suite has 19 passing tests, including parsing fixtures, wrapper arguments,
private environment propagation, native main-thread selection, recovery
idempotence, partial recovery retries, bundle tampering and compression hashes.
Initial failures and subsequent test logs remain under
`$REPO/.local/evidence/E28/harness-*`.

The short live check ran inside a drained H host window:

```sh
.local/bin/host-window quiet H 'E28 harness counters and Python tests'
python3 -B scripts/e11-tools-test
python3 -B scripts/e17-flat09-tests
python3 -B scripts/e28-platform-check
.local/bin/host-window release H
scripts/goliath-cpu restore
scripts/goliath-cpu restore
```

The two counter suites passed 7 and 5 tests. Four throwaway Python workers
reported allowed CPUs `0-3`, and one reported `4`. All five worker CPUs were
100% busy during the two-second steady window. Whole-cell CPU 5 busy time was
56.75%, the largest outside process averaged 0.3461 core, no throttle counter
rose, and 23 swap-in pages were recorded. Slice, governor and driver-cgroup
settings matched their originals after normal cleanup and after SIGTERM.
Both standalone restore calls were no-ops. The local record is
`$REPO/.local/evidence/E28/harness-live.json`.
For an independent check, the reviewer uses a fresh name such as
`python3 -B scripts/e28-platform-check --name repro-harness-live` in its own
drained H host window. The original record cannot be overwritten.

These are builder checks, not capacity results. Actual E28 bundle execution,
limiter-mode load, Darwin runtime regression checks, the soak and M3 remain
unproven until the reviewer runs them. No Rust build or game-server load ran as
part of the harness port.

## Fresh rebuild

Disposition: ready for reviewer measurements under the explicit
[gate disposition](data/E28/gate-disposition.json). All four Linux release
bundles pass `goliath_bundle.validate`. The canonical replacement was made
inside B's quiet reservation, then the reservation was released. No capacity
result is claimed by this rebuild.

The builder cloned the three requested upstream trees at the documented pins.
All canonical patch application checks passed, including the stamp overlay on
top of warm-tick. The internal sources match all 150 recorded E24/E26 file
comparisons and remain unchanged from repository HEAD. The protocol/world Git
dependency is clean at `0d21d5520ec26c49b774bf1533e53ea8af81278d`; its input
record contains 2,839 source-file hashes.

The separate stamp Linux patch enables `CLOCK_MONOTONIC_RAW` in tiles and bots
and enables the decode-before-dispatch test. The lint-only Meridian port was
rejected and removed. Its inputs and incompatible-MSRV Clippy failure remain
recorded as superseded. The documented seven-patch reactor tree was restored
byte-for-byte and all three proxy executables were rebuilt in release. Their
hashes exactly match the first canonical proxy build. Post-rebuild Clippy has
only the deprecated `fetch_update` diagnostic at `meridian-net/src/io_pool.rs:323`.
Root strict Clippy retains the same deprecation at
`crates/goliath-store/src/main.rs:345`; internal crates remain unchanged.
Additional whole-workspace formatting checks find historical E2 formatting;
the exact scoped proxy formatter passes. These four failed gate labels remain
failed and are accepted only by the reviewer's named toolchain-lint disposition.

Plain-09's resource-backed `flat_light` integration passes under `fast`.
Root tests pass all 82 cases. Final and lite pass their lease, warm-tick and
scoped Clippy gates; lite also passes its three instrumentation tests. The
stamped release lease gate completed with four passing Linux tests. Its fast
warm tests pass all three cases, bot tests pass all 17 cases, and reader tests
pass all eight cases, including decode-before-dispatch. Stamped tile, bot and
client-library strict Clippy gates pass under `fast`. Unstarted compiling gates
were switched to `fast`, with each gate's actual profile recorded.
Release is reserved for frozen executables. The native thread CPU clock probe
runs and reports `CLOCK_THREAD_CPUTIME_ID` with 1 ns resolution; this does not
qualify the instrumentation-cost bound.

[The build manifest](data/E28/build-manifest.json) hashes every bundled file.
All stored bytes match it, but a matching hash does not waive a failed gate.
Per-bundle build records retain failures, corrected attempts, profile settings,
ordered patch hashes and compiled source identities. Later gate evidence stays
outside the frozen bundles. The replacement anchor SHA-256 is
`7dd3a3e222d46b319eea6cc65fc586afbe1dc33afd14c8635a0f62111bd10347`.
The removed lint-only patch is absent from both the canonical source stack and
the patch directory. The real stamp clock port remains selected.

Linux's keeper uses BOOTTIME against MONOTONIC-backed `Instant` brackets. Its
slack is `(next.after - previous.before) / 500 + 1 microsecond`, rebased after
each accepted check. A reviewer-owned live smoke should retain lease traces
for at least 30 seconds, verify increasing renewal counters and send-anchored
250 ms cadence, and reject false clock anomalies or authority stops. A real
system suspend was not performed.

Disposable target directories and redundant staging copies were removed through
the measurement-aware B build window using scoped `scripts/clean-builds`, never
`--all`. This freed 10,256,855,296 apparent bytes, about 9.55 GiB. All three
manual source checkouts and the four frozen bundles remain. The workspace used
1,448,943,616 allocated bytes after cleanup, about 1.35 GiB; exact accounting is
in [the disk record](data/E28/build-disk.json). No commit or push was made.

## Plain-09 baseline

The initial builder preflight rejected the published plain-09
bundle with `E28 source equality or build gates did not pass`. Its build record
has `verification_status: failed` and retains `plain-09-fmt`. The manifest
existed and the host had no running or waiting builds at that preflight, but
those two scheduling prerequisites did not waive the validation failure.

The reviewer subsequently recorded [the gate disposition](data/E28/gate-disposition.json),
SHA-256 `0247ea9473e7cb4f07598f885fc0052dc3a48aaf593d7b22c6f5acb832ccfa5f`.
It accepts only the named, bundle-specific toolchain-lint failures for
`plain-09-fmt`, `root-clippy`, `proxy-fmt` and `proxy-clippy`. Tests, compile
errors, source deviations and any other failed gate still block. Plain-09 now
validates. Measurements below used this disposition; all results
remain pending reviewer reproduction.

The builder's baseline bracket is 200/225 players, pending reviewer reproduction.
Both 200-player repeats pass; both 225-player repeats are valid TPS/raw-P99
overloads. Single pilots at 150 and 250 and the 50-player smoke are retained
separately and do not count toward this staircase. The retained-data checker
passes geometry, movement, population, raw-tick and CPU/noise checks for all
four staircase rows. No rejected preflight is represented as an overload.

The direct single-cell Linux command now establishes the legacy baseline
quiet marker inside an existing host reservation. Both this command and the
cluster runner refuse to overwrite an existing compressed raw row. Regression
coverage includes marker setup, compressed-name protection and relative-path
compression. Unit checks do not waive release gates.

The original builder smoke command was:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-plain-50-r0'
python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e28/plain-09 --data-dir docs/experiments/data/E28/baseline --cpu-mode pin --cell 50 --repeat 0 --out docs/experiments/data/E28/baseline/e28-smoke-plain-50-r0.json
.local/bin/host-window release E28M
```

This smoke passed. Its relative-path compression failure occurred after the
measurement; the gzip was verified byte-exact, its decompressed hash was saved,
and only the duplicate uncompressed copy was removed. No measurement was rerun.
The path-handling bug is fixed and regression-tested. If interrupted, restore with
`python3 -B scripts/goliath-cpu restore` before handling stale host locks.
The completed baseline's raw rows and retained-data check are indexed by
`data/E28/baseline/staircase.json` and `verified-summary.json`.

### All baseline cells

| Cell | Players | Repeat | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [smoke-plain-50-r0](data/E28/baseline/e28-smoke-plain-50-r0.json.gz) | 50 | 0 | yes | yes | 19.993 | 8.740 | publication repaired; native exit 1 |
| [pilot-plain-150-r0](data/E28/baseline/e28-pilot-plain-150-r0.json.gz) | 150 | 0 | yes | yes | 19.993 | 29.200 | pass |
| [pilot-plain-250-r0](data/E28/baseline/e28-pilot-plain-250-r0.json.gz) | 250 | 0 | yes | no | 15.463 | 73.084 | TPS < 19.9 and raw P99 >= 50 ms |
| [baseline-200-r0](data/E28/baseline/e28-baseline-200-r0.json.gz) | 200 | 0 | yes | yes | 19.993 | 46.184 | pass |
| [baseline-200-r1](data/E28/baseline/e28-baseline-200-r1.json.gz) | 200 | 1 | yes | yes | 19.993 | 46.138 | pass |
| [baseline-225-r0](data/E28/baseline/e28-baseline-225-r0.json.gz) | 225 | 0 | yes | no | 19.488 | 57.091 | TPS < 19.9 and raw P99 >= 50 ms |
| [baseline-225-r1](data/E28/baseline/e28-baseline-225-r1.json.gz) | 225 | 1 | yes | no | 19.454 | 57.848 | TPS < 19.9 and raw P99 >= 50 ms |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady / whole | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [smoke-plain-50-r0](data/E28/baseline/e28-smoke-plain-50-r0.json.gz) | 0.167 | - | - | 0.017 | 2.84% | 21.25% | 0.156 / 0.165 | 430 |
| [pilot-plain-150-r0](data/E28/baseline/e28-pilot-plain-150-r0.json.gz) | 0.559 | - | - | 0.049 | 7.42% | 27.00% | 0.175 / 0.213 | 201 |
| [pilot-plain-250-r0](data/E28/baseline/e28-pilot-plain-250-r0.json.gz) | 1.064 | - | - | 0.069 | 10.63% | 21.40% | 0.162 / 0.165 | 10 |
| [baseline-200-r0](data/E28/baseline/e28-baseline-200-r0.json.gz) | 0.894 | - | - | 0.065 | 10.09% | 20.25% | 0.145 / 0.164 | 19 |
| [baseline-200-r1](data/E28/baseline/e28-baseline-200-r1.json.gz) | 0.902 | - | - | 0.066 | 10.32% | 22.86% | 0.183 / 0.171 | 27 |
| [baseline-225-r0](data/E28/baseline/e28-baseline-225-r0.json.gz) | 1.067 | - | - | 0.071 | 11.05% | 22.54% | 0.157 / 0.181 | 22 |
| [baseline-225-r1](data/E28/baseline/e28-baseline-225-r1.json.gz) | 1.068 | - | - | 0.070 | 10.99% | 19.82% | 0.163 / 0.161 | 6 |

Virtual plain tiles are in a/b/c/d order. These are observed player ranges across every sample, not four server processes. All rows have zero terrain-reader clients, 64 prescribed pigs per square and 256 live pigs at every sample.

| Cell | Virtual player ranges a / b / c / d |
| --- | --- |
| [smoke-plain-50-r0](data/E28/baseline/e28-smoke-plain-50-r0.json.gz) | 12..15 / 11..14 / 11..14 / 11..14 |
| [pilot-plain-150-r0](data/E28/baseline/e28-pilot-plain-150-r0.json.gz) | 34..40 / 34..41 / 34..41 / 33..40 |
| [pilot-plain-250-r0](data/E28/baseline/e28-pilot-plain-250-r0.json.gz) | 57..68 / 56..67 / 56..67 / 56..67 |
| [baseline-200-r0](data/E28/baseline/e28-baseline-200-r0.json.gz) | 45..53 / 45..54 / 45..54 / 45..54 |
| [baseline-200-r1](data/E28/baseline/e28-baseline-200-r1.json.gz) | 45..54 / 45..54 / 45..54 / 45..53 |
| [baseline-225-r0](data/E28/baseline/e28-baseline-225-r0.json.gz) | 51..61 / 51..61 / 51..61 / 50..60 |
| [baseline-225-r1](data/E28/baseline/e28-baseline-225-r1.json.gz) | 51..61 / 51..60 / 51..61 / 50..60 |

## Cluster frontier

The completed v3 tested bracket is 425/475. Both 425 rows pass, 450 is mixed,
and both 475 rows fail raw P99. No extra 450 repeat ran. This is the same
unstable-band rule as E20: 425 is the highest stable tested pass, not a claim
that every count above it fails. The ratio to this machine's 200 baseline is
2.125x, pending reviewer reproduction.

All eight v3 cells are valid. CPU 4 busy ranges from 29.30% to 40.36%, and CPU 5
from 45.72% to 53.40%; bots are not late. The two upper rows use 3.220 and
3.238 server cores and still miss the strict raw-P99 rule. Average server CPU
need not reach four cores before the tick-tail limit fails. No harness-core or
bot-core saturation is claimed as a cluster failure.

The replacement final bundle validates under the explicit gate disposition.
The manifest and all proxy source-stack records exclude the rejected lint-only
Meridian Linux patch. The 100-player cluster smoke passes, with all four tiles
at 20 TPS, 64 pigs per tile, correct cpusets and restored slices/governors.
All cluster results remain pending reviewer reproduction.

The original staircase is now **harness-v1 evidence only**. Both 400-player
repeats and both 425-player repeats pass, with one invalid 400/r1 startup and
its explicitly named retry retained. The final v1 row, 450/r0, passes as well.
It uses only 2.587 server cores while CPU 5 is 94.13% busy. At 425/r1 CPU 5 is
92.76% busy and servers use 2.412 cores. Continuing that staircase would risk
measuring the test-only shims' core limit before reaching the server frontier.
No v1 row will determine the reported frontier.

The original builder cluster smoke command was:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-final-100-r0'
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/frontier --cpu-mode pin --counts 100 --name e28-smoke-final-100-r0
.local/bin/host-window release E28M
```

This historical smoke used harness-v1, whose hashes are in its raw row. The
command above records that run, not a v3 reproduction. Final reproduction uses
the explicit `--direct-bots` commands below. The local resume record is
`$REPO/.local/evidence/E28/measure-state.md`.

### Forward-only capacity shim and harness-v2

The reviewer requested this change after the 450/r0 row completed. The capacity
pass rule consumes shim errors only, not shim packet counts. Proxy path
histograms are separate and unchanged. Capacity shims now opt into
`--forward-only`: `HostPrelude` rewrites the initial host as before, then a
large-buffer `recv`/`send` loop forwards opaque bytes without constructing or
feeding `WitnessDecoder`. Chunks go directly to the peer when its output queue
is empty; only unsent bytes wait for write readiness. The 8 MiB pending cap,
error reporting, forwarded-byte
totals and outbound-connection count remain. Decoded witness mode stays available
for callers that actually request witnesses.

Queued output drains before a half-close is propagated. Partial writes preserve
order. The handshake size guard applies to the handshake frame, not a valid
coalesced opaque tail. Twelve tests cover fragmented rewriting, large coalesced
tails, byte-exact forwarding in both directions, no decoder construction,
partial writes, half-close/close handling, pending-cap enforcement and malformed
handshake rejection, empty probe closure, and immediate full/partial/blocked writes. Existing E17,
E18, frontier and platform checks also pass.

The single `e28-shimcheck-425-r0` diagnostic passes. All five shims report
forward-only mode with no forwarding errors. Settings restoration passes.

| Metric | Harness-v1 425/r1 | Forward-only diagnostic 425/r0 |
| --- | ---: | ---: |
| CPU 5 busy | 92.76% | 89.04% |
| Five shims, steady cores | 0.579295 | 0.533147 |
| Server group, steady cores | 2.411888 | 2.613344 |

This is one diagnostic, not a staircase repeat or a claim about an exact saving.
Its raw row and the v1 control are retained. The frontier restarts at 400 under
`e28-frontier-v2-<count>-r<repeat>` names. Both 400-player repeats pass. The
reviewer then stopped v2 because its modest CPU 5 reduction would still risk a
harness-core limit before server failure. The already-running 400/r1 finished
without abort; no more v2 cells ran. Every v1/v2 row remains in the report,
but neither generation determines the frontier or ratio.

### Direct routing and harness-v3

V3 removes the test-only routing hop entirely. On the Mac, the front shims were
outside both the server and bot caps too. This changes that test-only hop, not
the server budget: the same final tiles, proxy, store and sidecars still share
CPUs 0 to 3, and all five native bot groups still share CPU 4. Layout, seeds,
routes, speed, height, view distance, warmup, measurement and pass rule stay
unchanged. Each group connects directly to the proxy with
`--handshake-host <group host>`.

Only the bot executable comes from final-stamps, SHA-256
`73a2c53b02ecaf065942381cfdb170d74f205638a61aed3cd1ddb432c9aa4848`.
It is validated independently against that entry in the Linux build manifest,
before and after every cell. Each row records `bots_bundle: final-stamps` and
the actual bot identity; server binaries remain those of final.

Tracing is environment-opt-in. With `E25_LATENCY_DIR` absent, `Trace::bind`
returns `None` before output setup. The added per-write clock call is guarded
by optional trace selection, and queued/accepted trace work is guarded as well.
Bot children explicitly receive none of `E25_LATENCY_DIR`, `E25_LATENCY_NAMES`
or `E25_LATENCY_TILE`. Ordinary pre-existing movement/telemetry timers remain;
no instrumentation-cost bound is claimed here.

[The disabled-trace proof](data/E28/direct-bots-proof.json) reconstructs the
stamped sources from the recorded patches and matches all three frozen source
hashes. Its native test executes the original bind/write guards with a counted
clock and substituted socket/JSON sinks: disabled makes zero clock calls;
the enabled positive control makes one. This is a guard test, not a rebuild of
the frozen bot. No v3 cell starts before that proof passes.

The single `e28-direct-425-r0` check passes, with no shim processes, the selected
stamped bot hash in the row, no tracing enabled and all five intended handshake
hosts. All tiles pass TPS/raw P99, bots have no late rounds, and restoration
passes. Its comparison with v1's 425/r1 is:

| Metric | Harness-v1 425/r1 | Direct check 425/r0 |
| --- | ---: | ---: |
| CPU 5 busy | 92.76% | 46.79% |
| CPU 4 busy | 29.29% | 31.73% |
| Bot group, steady cores | 0.245732 | 0.275351 |
| Server group, steady cores | 2.411888 | 2.606961 |

This is one diagnostic comparison, not a staircase repeat. Eight staircase
cells then ran from 400 through 475 under `e28-frontier-v3-<count>-r<repeat>`
names, two repeats per step. Only those v3 rows qualify the frontier. CPU 4 did
not saturate and bots did not go late, so the reviewer stop condition was not
triggered. No further load cells ran after the bracket closed.

### Complete cell tables

Every cell is retained, including the invalid startup. TPS and raw P99 use tile order a / b / c / d. CPU values are steady process counters; CPU 4/5 busy and swap-in cover D12's cell-start through steady-end window. Outside totals show both steady and whole-cell phases. A dash means unavailable, never zero. The invalid startup has no steady window, so its CPU 5 and swap-in values describe startup only. All completed cluster rows have zero dropped raw ticks, all requested bots online without late rounds, and 64 pigs per tile before and after measurement.

#### Harness-v1, retained only

| Cell | Players | Repeat | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [smoke-final-100-r0](data/E28/frontier/e28-smoke-final-100-r0.json.gz) | 100 | 0 | yes | yes | 20.002 / 20.003 / 20.004 / 20.005 | 7.719 / 7.841 / 8.091 / 7.599 | pass |
| [frontier-400-r0](data/E28/frontier/e28-frontier-400-r0.json.gz) | 400 | 0 | yes | yes | 19.994 / 19.997 / 19.998 / 19.998 | 29.966 / 31.732 / 32.200 / 31.685 | pass |
| [frontier-400-r1](data/E28/frontier/e28-frontier-400-r1.json) | 400 | 1 | no | no | - | - | startup IO-pool log race; no measurement |
| [frontier-400-r1-a1](data/E28/frontier/e28-frontier-400-r1-a1.json.gz) | 400 | 1 | yes | yes | 19.992 / 20.007 / 20.007 / 19.995 | 29.938 / 30.357 / 31.350 / 32.110 | pass |
| [frontier-425-r0](data/E28/frontier/e28-frontier-425-r0.json.gz) | 425 | 0 | yes | yes | 20.003 / 20.005 / 20.003 / 20.001 | 35.628 / 32.745 / 37.401 / 35.706 | pass |
| [frontier-425-r1](data/E28/frontier/e28-frontier-425-r1.json.gz) | 425 | 1 | yes | yes | 20.003 / 20.003 / 20.001 / 20.000 | 39.330 / 37.856 / 35.566 / 38.627 | pass |
| [frontier-450-r0](data/E28/frontier/e28-frontier-450-r0.json.gz) | 450 | 0 | yes | yes | 20.006 / 19.995 / 20.003 / 20.008 | 40.510 / 46.583 / 42.956 / 43.229 | pass |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady / whole | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [smoke-final-100-r0](data/E28/frontier/e28-smoke-final-100-r0.json.gz) | 0.560 | 0.142 | 217..220 | 0.047 | 7.21% | 52.23% | 0.146 / 0.174 | 3330 |
| [frontier-400-r0](data/E28/frontier/e28-frontier-400-r0.json.gz) | 2.259 | 0.591 | 821..826 | 0.237 | 27.80% | 92.32% | 0.179 / 0.203 | 661 |
| [frontier-400-r1](data/E28/frontier/e28-frontier-400-r1.json) | - | - | - | - | 0.24% | 21.00% | - / - | 7 |
| [frontier-400-r1-a1](data/E28/frontier/e28-frontier-400-r1-a1.json.gz) | 2.246 | 0.576 | 821..826 | 0.228 | 26.87% | 93.20% | 0.179 / 0.180 | 1170 |
| [frontier-425-r0](data/E28/frontier/e28-frontier-425-r0.json.gz) | 2.408 | 0.624 | 871..877 | 0.249 | 29.78% | 92.10% | 0.184 / 0.182 | 262 |
| [frontier-425-r1](data/E28/frontier/e28-frontier-425-r1.json.gz) | 2.412 | 0.622 | 872..877 | 0.246 | 29.29% | 92.76% | 0.192 / 0.194 | 35 |
| [frontier-450-r0](data/E28/frontier/e28-frontier-450-r0.json.gz) | 2.587 | 0.655 | 923..927 | 0.271 | 31.57% | 94.13% | 0.195 / 0.227 | 1594 |

#### Harness-v2 and forward-only diagnostic, retained only

| Cell | Players | Repeat | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [shimcheck-425-r0](data/E28/frontier/e28-shimcheck-425-r0.json.gz) | 425 | 0 | yes | yes | 20.003 / 19.997 / 20.008 / 19.992 | 39.219 / 37.114 / 38.425 / 36.003 | pass |
| [frontier-v2-400-r0](data/E28/frontier/e28-frontier-v2-400-r0.json.gz) | 400 | 0 | yes | yes | 20.005 / 19.996 / 19.994 / 20.011 | 30.346 / 31.532 / 29.576 / 31.042 | pass |
| [frontier-v2-400-r1](data/E28/frontier/e28-frontier-v2-400-r1.json.gz) | 400 | 1 | yes | yes | 20.005 / 20.003 / 20.003 / 20.005 | 33.301 / 34.232 / 36.583 / 36.017 | pass |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady / whole | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [shimcheck-425-r0](data/E28/frontier/e28-shimcheck-425-r0.json.gz) | 2.613 | 0.697 | 872..875 | 0.300 | 34.08% | 89.04% | 0.179 / 0.175 | 13 |
| [frontier-v2-400-r0](data/E28/frontier/e28-frontier-v2-400-r0.json.gz) | 2.405 | 0.663 | 821..827 | 0.288 | 32.13% | 88.77% | 0.182 / 0.192 | 5 |
| [frontier-v2-400-r1](data/E28/frontier/e28-frontier-v2-400-r1.json.gz) | 2.443 | 0.657 | 821..828 | 0.291 | 32.95% | 88.98% | 0.192 / 0.188 | 385 |

#### Direct-routing diagnostic, excluded from the staircase

| Cell | Players | Repeat | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [direct-425-r0](data/E28/frontier/e28-direct-425-r0.json.gz) | 425 | 0 | yes | yes | 19.996 / 19.998 / 19.999 / 19.986 | 34.873 / 33.416 / 32.256 / 34.227 | pass |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady / whole | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [direct-425-r0](data/E28/frontier/e28-direct-425-r0.json.gz) | 2.607 | 0.682 | 871..877 | 0.275 | 31.73% | 46.79% | 0.212 / 0.208 | 1297 |

#### Harness-v3, qualifying staircase

| Cell | Players | Repeat | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [frontier-v3-400-r0](data/E28/frontier/e28-frontier-v3-400-r0.json.gz) | 400 | 0 | yes | yes | 20.000 / 20.000 / 20.000 / 20.000 | 27.962 / 28.647 / 28.755 / 30.784 | pass |
| [frontier-v3-400-r1](data/E28/frontier/e28-frontier-v3-400-r1.json.gz) | 400 | 1 | yes | yes | 20.007 / 20.007 / 19.991 / 19.993 | 31.274 / 29.552 / 29.776 / 29.256 | pass |
| [frontier-v3-425-r0](data/E28/frontier/e28-frontier-v3-425-r0.json.gz) | 425 | 0 | yes | yes | 20.004 / 20.004 / 20.004 / 20.002 | 40.502 / 39.937 / 39.708 / 38.485 | pass |
| [frontier-v3-425-r1](data/E28/frontier/e28-frontier-v3-425-r1.json.gz) | 425 | 1 | yes | yes | 20.001 / 19.989 / 20.006 / 19.989 | 37.541 / 35.610 / 35.242 / 35.248 | pass |
| [frontier-v3-450-r0](data/E28/frontier/e28-frontier-v3-450-r0.json.gz) | 450 | 0 | yes | no | 20.003 / 20.002 / 20.000 / 20.000 | 49.845 / 50.151 / 52.963 / 52.403 | raw P99 >= 50 ms; no drops |
| [frontier-v3-450-r1](data/E28/frontier/e28-frontier-v3-450-r1.json.gz) | 450 | 1 | yes | yes | 20.004 / 19.989 / 20.007 / 19.991 | 46.151 / 42.084 / 44.055 / 42.628 | pass |
| [frontier-v3-475-r0](data/E28/frontier/e28-frontier-v3-475-r0.json.gz) | 475 | 0 | yes | no | 19.996 / 19.997 / 19.997 / 20.011 | 65.538 / 60.460 / 63.468 / 64.048 | raw P99 >= 50 ms; no drops |
| [frontier-v3-475-r1](data/E28/frontier/e28-frontier-v3-475-r1.json.gz) | 475 | 1 | yes | no | 20.001 / 20.008 / 20.008 / 19.995 | 62.699 / 62.206 / 60.775 / 64.462 | raw P99 >= 50 ms; no drops |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady / whole | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [frontier-v3-400-r0](data/E28/frontier/e28-frontier-v3-400-r0.json.gz) | 2.425 | 0.645 | 821..826 | 0.253 | 29.30% | 45.72% | 0.196 / 0.192 | 538 |
| [frontier-v3-400-r1](data/E28/frontier/e28-frontier-v3-400-r1.json.gz) | 2.429 | 0.650 | 822..827 | 0.257 | 30.18% | 46.55% | 0.197 / 0.205 | 8 |
| [frontier-v3-425-r0](data/E28/frontier/e28-frontier-v3-425-r0.json.gz) | 2.668 | 0.698 | 871..877 | 0.284 | 32.65% | 46.05% | 0.203 / 0.201 | 8 |
| [frontier-v3-425-r1](data/E28/frontier/e28-frontier-v3-425-r1.json.gz) | 2.620 | 0.692 | 872..877 | 0.278 | 32.18% | 46.74% | 0.185 / 0.196 | 24 |
| [frontier-v3-450-r0](data/E28/frontier/e28-frontier-v3-450-r0.json.gz) | 2.976 | 0.795 | 922..927 | 0.329 | 36.44% | 51.22% | 0.255 / 0.225 | 2 |
| [frontier-v3-450-r1](data/E28/frontier/e28-frontier-v3-450-r1.json.gz) | 2.919 | 0.780 | 922..927 | 0.324 | 36.53% | 51.19% | 0.231 / 0.224 | 25 |
| [frontier-v3-475-r0](data/E28/frontier/e28-frontier-v3-475-r0.json.gz) | 3.220 | 0.851 | 973..978 | 0.364 | 40.02% | 53.40% | 0.241 / 0.232 | 1 |
| [frontier-v3-475-r1](data/E28/frontier/e28-frontier-v3-475-r1.json.gz) | 3.238 | 0.858 | 973..978 | 0.360 | 40.36% | 50.19% | 0.223 / 0.219 | 2 |

### Per-tile populations

The following vectors use a / b / c / d order and show before -> after. Placed and held players are separate; a crossing can have a held destination. Readers are not counted as placed players. All completed cells retain 64 live pigs per tile at both endpoints. The invalid startup has no population window.

| Cell | Placed players | Held players | Terrain readers |
| --- | --- | --- | --- |
| [smoke-final-100-r0](data/E28/frontier/e28-smoke-final-100-r0.json.gz) | 26 / 23 / 27 / 24 -> 23 / 28 / 24 / 25 | 0 / 1 / 0 / 2 -> 0 / 2 / 1 / 0 | 10 / 10 / 10 / 10 -> 10 / 10 / 10 / 10 |
| [frontier-400-r0](data/E28/frontier/e28-frontier-400-r0.json.gz) | 104 / 104 / 92 / 100 -> 95 / 95 / 106 / 104 | 2 / 1 / 0 / 1 -> 2 / 0 / 3 / 0 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [frontier-400-r1](data/E28/frontier/e28-frontier-400-r1.json) | - | - | - |
| [frontier-400-r1-a1](data/E28/frontier/e28-frontier-400-r1-a1.json.gz) | 105 / 104 / 93 / 99 -> 98 / 94 / 109 / 99 | 3 / 1 / 0 / 2 -> 2 / 0 / 3 / 1 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [frontier-425-r0](data/E28/frontier/e28-frontier-425-r0.json.gz) | 114 / 106 / 97 / 108 -> 107 / 99 / 117 / 103 | 2 / 1 / 0 / 4 -> 4 / 0 / 2 / 1 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-425-r1](data/E28/frontier/e28-frontier-425-r1.json.gz) | 114 / 105 / 97 / 109 -> 107 / 99 / 117 / 102 | 2 / 2 / 0 / 4 -> 4 / 0 / 2 / 2 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-450-r0](data/E28/frontier/e28-frontier-450-r0.json.gz) | 119 / 110 / 101 / 120 -> 126 / 106 / 111 / 107 | 2 / 1 / 1 / 3 -> 1 / 3 / 1 / 1 | 45 / 45 / 45 / 45 -> 45 / 45 / 45 / 45 |
| [shimcheck-425-r0](data/E28/frontier/e28-shimcheck-425-r0.json.gz) | 114 / 105 / 98 / 109 -> 105 / 99 / 119 / 103 | 2 / 1 / 0 / 2 -> 4 / 0 / 1 / 1 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-v2-400-r0](data/E28/frontier/e28-frontier-v2-400-r0.json.gz) | 105 / 103 / 93 / 99 -> 98 / 94 / 109 / 99 | 2 / 1 / 0 / 2 -> 2 / 0 / 3 / 1 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [frontier-v2-400-r1](data/E28/frontier/e28-frontier-v2-400-r1.json.gz) | 107 / 99 / 90 / 105 -> 103 / 91 / 111 / 96 | 3 / 1 / 0 / 3 -> 4 / 0 / 1 / 2 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [direct-425-r0](data/E28/frontier/e28-direct-425-r0.json.gz) | 112 / 110 / 99 / 105 -> 104 / 103 / 102 / 116 | 2 / 1 / 0 / 2 -> 1 / 2 / 3 / 2 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-v3-400-r0](data/E28/frontier/e28-frontier-v3-400-r0.json.gz) | 104 / 105 / 94 / 98 -> 101 / 97 / 93 / 110 | 3 / 0 / 0 / 2 -> 1 / 1 / 1 / 1 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [frontier-v3-400-r1](data/E28/frontier/e28-frontier-v3-400-r1.json.gz) | 104 / 105 / 93 / 98 -> 94 / 96 / 105 / 105 | 3 / 1 / 0 / 2 -> 1 / 0 / 2 / 0 | 40 / 40 / 40 / 40 -> 40 / 40 / 40 / 40 |
| [frontier-v3-425-r0](data/E28/frontier/e28-frontier-v3-425-r0.json.gz) | 114 / 107 / 98 / 106 -> 103 / 104 / 105 / 113 | 3 / 1 / 0 / 4 -> 1 / 1 / 3 / 2 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-v3-425-r1](data/E28/frontier/e28-frontier-v3-425-r1.json.gz) | 112 / 109 / 99 / 105 -> 105 / 103 / 102 / 115 | 2 / 1 / 0 / 2 -> 1 / 1 / 3 / 2 | 42 / 42 / 42 / 42 -> 42 / 42 / 42 / 42 |
| [frontier-v3-450-r0](data/E28/frontier/e28-frontier-v3-450-r0.json.gz) | 121 / 110 / 102 / 117 -> 109 / 108 / 118 / 116 | 2 / 1 / 0 / 4 -> 2 / 0 / 3 / 1 | 45 / 45 / 45 / 45 -> 45 / 45 / 45 / 45 |
| [frontier-v3-450-r1](data/E28/frontier/e28-frontier-v3-450-r1.json.gz) | 120 / 111 / 102 / 117 -> 108 / 109 / 116 / 117 | 2 / 1 / 0 / 4 -> 1 / 0 / 3 / 2 | 45 / 45 / 45 / 45 -> 45 / 45 / 45 / 45 |
| [frontier-v3-475-r0](data/E28/frontier/e28-frontier-v3-475-r0.json.gz) | 122 / 116 / 110 / 127 -> 120 / 110 / 128 / 118 | 3 / 0 / 1 / 3 -> 4 / 0 / 2 / 1 | 47 / 47 / 47 / 47 -> 47 / 47 / 47 / 47 |
| [frontier-v3-475-r1](data/E28/frontier/e28-frontier-v3-475-r1.json.gz) | 123 / 117 / 109 / 127 -> 115 / 112 / 127 / 121 | 2 / 0 / 1 / 2 -> 2 / 0 / 3 / 1 | 47 / 47 / 47 / 47 -> 47 / 47 / 47 / 47 |

### Readback, hashes and exact reproduction

[The complete ledger](data/E28/measurement-ledger.json) records every raw path, decompressed-byte and stored-file SHA-256, exact builder command, harness generation, outcome, CPU counters, populations, and restoration check. Every raw JSON over 1 MB is `.json.gz`; no samples were trimmed. The failed startup's native evidence is retained and hashed separately in that ledger. The first plain smoke's native exit 1 was a publication error after a passing measurement, repaired byte-exact without a rerun. The failed IO-pool startup was a readiness race: the proxy binds TCP before starting its pool, and the old launcher returned on TCP connect. The runner now waits at most 10 seconds for the complete pool message and rejects early proxy exit. Its failed row and explicit `-a1` retry remain separate.

Retained-data checks reproduce the baseline bracket and v3 endpoint arithmetic:

```sh
python3 -B scripts/e17-flat09-report --data-dir docs/experiments/data/E28/baseline
python3 -B scripts/e20-results e28-frontier-v3 --baseline 200 --data-dir docs/experiments/data/E28/frontier
```

The reviewer runs fresh cells from the repository root after validating both selected bundles and independently checking the disabled-trace guard. The guard test uses a tiny native probe through the build reservation, so run it before acquiring a measurement window. These commands have not been run by the builder and create only reviewer-owned `repro-*` results:

```sh
export GOLIATH_WINDOW_OWNER=E28R
python3 -B scripts/e28-direct-bots-tests
for e28_count in 200 225; do
  for e28_repeat in 0 1; do
    e28_label="repro-e28-baseline-${e28_count}-r${e28_repeat}"
    .local/bin/host-window quiet E28R "$e28_label"
    python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e28/plain-09 --data-dir docs/experiments/data/E28/reviewer-baseline --cpu-mode pin --cell "$e28_count" --repeat "$e28_repeat" --out "docs/experiments/data/E28/reviewer-baseline/${e28_label}.json"
    .local/bin/host-window release E28R
  done
done
for e28_count in 400 425 450 475; do
  for e28_repeat in 0 1; do
    e28_label="repro-e28-frontier-v3-${e28_count}-r${e28_repeat}"
    .local/bin/host-window quiet E28R "$e28_label"
    python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/reviewer-frontier --cpu-mode pin --direct-bots --counts "$e28_count" --name "$e28_label"
    .local/bin/host-window release E28R
  done
done
python3 -B scripts/e20-results repro-e28-frontier-v3 --baseline 200 --data-dir docs/experiments/data/E28/reviewer-frontier
```

Each cell stays in the foreground. Keep invalid and failed reproductions; do not repeat a mixed step. If interrupted, run `python3 -B scripts/goliath-cpu restore` before removing stale host locks. If the reproduced baseline differs, supply that value to readback rather than silently reusing 200. No `repro-*` data was created by the builder.

The source proof and all numerical cross-checks are builder evidence, not reviewer reproduction. The 30-minute soak, proxy latency, instrumentation-cost bound and optional M3 campaign are outside this capacity task and remain unclaimed here. No commit or push was made. Progress and decisions were not edited by the measurement builder.

### Final verification and handoff

[The verification record](data/E28/verification.json) lists 239 registered test
cases: 232 pass, seven explicitly skip retained Mac artifacts, and none fail.
The skips are one E23 bundle check, four E25 private index checks and two E26b
bundle checks. They are not passing validations of those artifacts. Unit
fixture output from soak/crash tests is not a live soak or M3 campaign.

The retained-data audit checks all 26 rows' decompressed and stored hashes,
redaction, D12 fields, cpusets and saved restoration policies. It recomputes
cluster TPS/raw P99 and baseline geometry/movement, checks zero raw-tick drops,
all bots online without late rounds, 64 pigs per tile, the stamped bot identity,
the disabled-trace proof, and one unchanged v3 harness identity. No raw samples
were trimmed. All private run/world folders were removed after extraction.
At the builder handoff, the host window was free; cleanup freed 0 MB and left
the remaining target under its cap. No builder measurement is queued. The
reviewer has since started independent reproduction as E28R. Those files are
not inspected or changed by this write-up.

Three harness failures were resolved without hiding results: the initial
single-cell marker gap, relative-path compression after the passing plain
smoke, and the IO-pool readiness race before the invalid cluster startup.
The native disabled-trace probe initially had a test-module nesting error;
the unchanged method was put in its original module context and the guard
tests then passed. No application or frozen binary was changed for that fix.

Reviewer reproduction is the remaining acceptance step. A proposed progress
update, not applied to progress or decisions, is: "E28 builder evidence under
D12 pinning gives a 200-player matched baseline and a 425-player stable v3
cluster step, or 2.125x. Baseline fails twice at 225; cluster 450 is mixed and
475 fails twice on raw P99. Pending independent reviewer reproduction."

## Proxy latency

Status: preregistered, awaiting the reviewer's last frontier cell and a free
host window. Results will remain pending reviewer reproduction.

The [latency and lite protocol](data/E28/latency-lite-protocol.json), SHA-256
`589514a75eacb3ebaa4fcc93d7f4c671e4ddae5ca080fadfc66efd473053443b`, fixes
the cell order before any observation. The [425-player witness plan](data/E28/latency/e28-latency-plan-r1.json)
has SHA-256 `509025575ab8deda84c02ae01b361c285e61390ff0261e258cc93fe9592100c3`.
It reuses E25's seed-25 selection over this load's original D9 cohorts.
The witnesses are A1/A48, B27/B39, C60/C81 and D5/D32, each prefixed
`E17Bot`. The plan retains their exact seeded anchors and cohort populations.
There are 425 real players, including 42 continuous crossers, and 256 pigs.

The E25 estimand is unchanged: the complete via-proxy bot-write to tile-decode
distribution is a conservative upper bound on the proxy contribution. Its
nearest-rank overall P95 must be strictly below 5 ms, with unmatched or
ambiguous sends at infinity. Every tile's P95 is reported separately.
The fixed window is 60 seconds after 30 seconds warmup, followed by two
seconds of decode drain. No forwarding tap, handshake shim or login relay
is present. Nothing subtracts STOP pauses or direct-reference quantiles.
This covers selected interior movement, not crossing latency or all packets.

The reviewer can reproduce the fixed order from the repository root with
fresh reviewer-owned names. The runner acquires and releases each window
under the exported owner; do not wrap a second owner around it.

```sh
export GOLIATH_WINDOW_OWNER=E28R
for e28_spec in pin-direct-r0 pin-via-r0 limiter-via-r0 limiter-direct-r0 limiter-via-r1 pin-via-r1; do
  e28_mode="${e28_spec%%-*}"
  e28_direct=()
  case "$e28_spec" in *-direct-*) e28_direct=(--direct-reference);; esac
  python3 -u -B scripts/e25-latency-cell-r2 --name "repro-e28-latency-${e28_spec}" --bins .local/bin/goliath-e28/final-stamps --data-dir docs/experiments/data/E28/reviewer-latency --players 425 --cpu-mode "$e28_mode" --plan docs/experiments/data/E28/latency/e28-latency-plan-r1.json --stop-brackets "${e28_direct[@]}"
  python3 -B scripts/e25-stop-overlap --summary "docs/experiments/data/E28/reviewer-latency/repro-e28-latency-${e28_spec}-summary.json" --data-dir docs/experiments/data/E28/reviewer-latency --name "repro-e28-latency-${e28_spec}-overlap"
done
```

If a summary is compressed, pass its actual `.json.gz` path to overlap
readback. Keep failed or invalid rows; no loop implies permission to overwrite
them or replace them with another repeat.

Two via rows per CPU mode are planned. The comparison mode keeps servers
and bots together on CPUs 0 to 4 under the original four-core and one-core
SIGSTOP caps. Pinning separates servers onto CPUs 0 to 3 and bots onto CPU 4.
Both modes put everything else on CPU 5 and retain every D12 validity rule.
The [counter-port record](data/E28/latency-lite-counter-port.json) fixes the
Linux comparison limiter's native aggregate process-CPU reader before any
builder observation. It avoids a Python walk of every `/proc` thread on
every budget sample. The existing token bucket, sampling interval, signal
calls, PID membership and caps are inherited unchanged. A native test covers
exited-thread CPU and comparison with process counters; no saturation cell
was run and discarded. Capacity runners and Darwin counters are unchanged.
One direct reference per mode replaces only the eight witnesses' destination,
using the normal authenticated initial-login path. Those rows retain 417
proxied sessions and all 42 crossers. E25 needs no direct subtraction to test
its conservative bound. The references diagnose baseline cost and cannot
repair a failed via result.

## Tick-cost lite rows

Status: preregistered, not yet measured, pending reviewer reproduction.

The same protocol fixes a fresh calibration of final-lite's native thread
clock probe, four final-stack untimed A controls and two final-lite B rows,
all at 425 players under pinning. It uses E26b's final-five-warmup-seconds
busiest-tile selection and its complete-cycle subset. The selected native
clock image must match the fresh calibration; Mac calibration and controls
are rejected. All bot groups connect directly with tracing disabled.

Each B readback uses all four same-platform A controls. Every clock read
and the full positive main-CPU delta are charged against warming before
testing a warming-only lower bound. Main CPU, raw P50 and raw P99 must each
remain inside the A controls' observed range under E26b's rules. An
out-of-spread row stays in the report and is not replaced. The lite recorder
covers warming only. A warming result below 5% cannot prove that the combined
additions are bounded below 5%.

Exact reviewer commands, in the preregistered order:

```sh
export GOLIATH_WINDOW_OWNER=E28R
python3 -u -B scripts/e26-clock-run --name repro-e28-lite-clock-r0 --bins .local/bin/goliath-e28/final-lite --data-dir docs/experiments/data/E28/reviewer-lite --cpu-mode pin
for e28_arm in a1 b1 a2 a3 b2 a4; do
  case "$e28_arm" in
    a*) e28_script=e26-lite-cell; e28_kind=A; e28_bundle=final;;
    b*) e28_script=e26b-final-lite-cell; e28_kind=B; e28_bundle=final-lite;;
  esac
  python3 -u -B "scripts/$e28_script" --name "repro-e28-lite-${e28_arm}" --arm "$e28_kind" --bins ".local/bin/goliath-e28/$e28_bundle" --data-dir docs/experiments/data/E28/reviewer-lite --players 425 --cpu-mode pin
done
for e28_b in b1 b2; do
  python3 -B scripts/e26b-repro-lite "repro-e28-lite-$e28_b" --data-dir docs/experiments/data/E28/reviewer-lite --controls repro-e28-lite-a1 repro-e28-lite-a2 repro-e28-lite-a3 repro-e28-lite-a4 --calibration docs/experiments/data/E28/reviewer-lite/repro-e28-lite-clock-r0.json --out "repro-e28-lite-${e28_b}-summary.json"
done
```

### Selected frozen identities

The trust anchor is SHA-256 `7dd3a3e222d46b319eea6cc65fc586afbe1dc33afd14c8635a0f62111bd10347`.
The disabled-trace proof is `5411d3d5039c3b6dccfd9e1a3da7eb78a30eb9347474f940aae7c0fd72f917ec`.
The manifest retains every bundled metadata and launcher hash as well. These are frozen-file identities, separate from the decompressed raw-row hashes in the ledger.

| Bundle | Used executable | SHA-256 |
| --- | --- | --- |
| plain-09 | `.local/bin/goliath-e28/plain-09/horizon-client` | `f9527d1acc54c25fd98efad4f8bdaec88c9d35a6051c1fb9c286e550eceaa629` |
| plain-09 | `.local/bin/goliath-e28/plain-09/horizon-bots` | `3a4874246ad92ec7dbd2f5a8e2280b14e25df4410615e229d30fef99f2b2c9e4` |
| final | `.local/bin/goliath-e28/final/horizon-client` | `1614a5f60b87b44e1cb08c28aae6925539dc30ff5572c9fe06e723cfe3f2e9a0` |
| final | `.local/bin/goliath-e28/final/meridian` | `085414788e7bd53c49346e22ee36924940a134bc44f05336f2a43d6af282fe2c` |
| final | `.local/bin/goliath-e28/final/meridian-sidecar` | `d4523dccd4b03f2d4c6b57909f9323bcfc271ff78e43cb1ee456f5e1f23976ad` |
| final | `.local/bin/goliath-e28/final/goliath-store` | `ac3c8075a6ec92e52a0201925a2b896c4ffb2803526473630bced68cd35ba888` |
| final-stamps | `.local/bin/goliath-e28/final-stamps/horizon-bots` | `73a2c53b02ecaf065942381cfdb170d74f205638a61aed3cd1ddb432c9aa4848` |
