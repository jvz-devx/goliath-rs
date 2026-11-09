# E28. Second platform: Linux with core pinning

Status: planned. Nothing is measured yet. The reviewer reproduces each result before it counts.

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

None yet.

## Harness port

The Linux port is implemented. Game-server cells have not run, and the reviewer
has not reproduced the port checks yet. Darwin's libproc and mach counter paths
remain in place.

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
validates. Measurements are proceeding under this disposition; all results
remain pending reviewer reproduction.

Zero smoke, pilot or staircase cells ran. There are no measured player counts,
TPS, raw P99, core counts, CPU 5 busy fractions, outside totals or swap-in
values to report. The baseline bracket and its reviewer reproduction remain
unproven. No rejected preflight is represented as a measured overload.

The direct single-cell Linux command now establishes the legacy baseline
quiet marker inside an existing host reservation. Both this command and the
cluster runner refuse to overwrite an existing compressed raw row. Five D9,
eight frontier, nineteen platform, five plain-metric and twelve E17 tool tests
pass. These checks do not validate the rejected release bundles.

After a valid replacement handoff, the prepared builder smoke command is:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-plain-50-r0'
python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e28/plain-09 --data-dir docs/experiments/data/E28/baseline --cpu-mode pin --cell 50 --repeat 0 --out docs/experiments/data/E28/baseline/e28-smoke-plain-50-r0.json
.local/bin/host-window release E28M
```

This command has not run. If interrupted, restore with
`python3 -B scripts/goliath-cpu restore` before handling stale host locks.
The next steps are smoke verification, separately labelled single pilot cells,
then two repeats per 25-player staircase step under the stated frontier rule.

## Cluster frontier

Not started. The final bundle's preflight is blocked by its failed gate record,
including `proxy-clippy`, `proxy-fmt` and `root-clippy`. See Fresh rebuild for
the required reviewer disposition. No guard was weakened and no release
artifact was modified by the measurement builder.

Zero cluster cells ran. CPU placement, 20 TPS operation, the D12 counters and
post-cell slice/governor restoration on these game binaries are unproven.
There is no cluster bracket or frontier-to-baseline ratio. After smoke and a
measured baseline, the cluster staircase starts near twice that baseline and
uses 25-player steps, retaining every failed, invalid and mixed row.

After a valid replacement handoff, the prepared builder cluster smoke is:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-final-100-r0'
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/frontier --cpu-mode pin --counts 100 --name e28-smoke-final-100-r0
.local/bin/host-window release E28M
```

This command has not run. The reviewer can independently execute the first two
smokes with these fresh names after the handoff is valid. No reproduction
directory or result has been created by the builder:

```sh
export GOLIATH_WINDOW_OWNER=E28R
.local/bin/host-window quiet E28R 'repro-e28-smoke-plain-50-r0'
python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e28/plain-09 --data-dir docs/experiments/data/E28/reviewer-baseline --cpu-mode pin --cell 50 --repeat 0 --out docs/experiments/data/E28/reviewer-baseline/repro-e28-smoke-plain-50-r0.json
.local/bin/host-window release E28R
.local/bin/host-window quiet E28R 'repro-e28-smoke-final-100-r0'
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/reviewer-frontier --cpu-mode pin --counts 100 --name repro-e28-smoke-final-100-r0
.local/bin/host-window release E28R
```

The capacity tables and exact bracket-reproduction counts are pending actual
measurements and reviewer reproduction. Raw rows over 1 MB will be retained as
`.json.gz` with decompressed-byte hashes. The local resume record is
`$REPO/.local/evidence/E28/measure-state.md`.
