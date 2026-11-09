# E28. Second platform: Linux with core pinning

Status: the reviewer reproduced the baseline (200 players), the cluster
frontier (425, a ratio of 2.125x), the proxy latency rows and the lite rows.
The 30-minute soak failed on `final` and again on `final-r2`, the tile bundle
with the two soak fixes. The log diagnostic and the frontier check on
`final-r2` are builder results. M3 is paused at 72 of 1,000 seeds on `final`
and deferred on `final-r2`. An independent audit found harness defects; none
changes the baseline, the frontier or the ratio. They are listed under
[Harness defects found by audit](#harness-defects-found-by-audit).

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

The plan as registered. Each step names its owner, and builders get exclusive files and checkouts.

1. **Harness port** (builder H). Add a platform layer to `scripts/`: `/proc`-based process and thread counters in place of libproc and mach, a `GOLIATH_RESOURCES` override for the resources path, the D12 cpuset layout, the system-slice narrowing and restore, frequency and throttle recording, and a `--cpu-mode pin|limiter` switch. Counters with no Linux equivalent are reported as null, never faked. Mac behaviour stays unchanged. The Python harness tests pass on Linux.
2. **Fresh rebuild** (builder B). Clone Horizon and Meridian from upstream at the README pins into `.local/src/{horizon,horizon-tiles,meridian}`, apply `patches/` in the documented order, and build release binaries with `CARGO_INCREMENTAL=0`, one target dir per workspace. Binaries: plain-09 server and bots; final tiles (FINAL-09, `horizon-13-lease-renewal-r2`, patch 10); the reactor-1 proxy, sidecar and e4 client; the bounded store from this repository; the lite variant (final tiles plus `horizon-21-lite-cpu-clean`); the latency-stamp tiles and bots (`horizon-15-...-lease-r2-b2`). Each is frozen under `.local/bin/goliath-e28/` with a build record of pins, patch hashes, gates and binary hashes. The Linux BOOTTIME path of the lease keeper is checked by build, lease tests and a short run. This is the first fresh-clone reconstruction of the stack.
3. **Plain-09 baseline staircase** under `pin`. Frontier rule: the highest step with two passes, closed by two fails one step above.
4. **Cluster frontier** on the final stack under `pin`, same rule.
5. **Proxy latency** with E25's no-extra-hop timestamps, at this machine's frontier, under `pin` and under `limiter` at the same load. The comparison tests D12's reason.
6. **Tick-cost lite rows** on the final stack, as in E26b.
7. **30-minute soak** at this machine's frontier.
8. **M3 campaign**, optional: 1,000 crash seeds, 12 partition and 6 stale-login cases.

Steps 1 and 2 run in parallel. Measurements run one at a time inside host windows.

What changed while it ran:

- The cluster frontier needed two harness changes before its rows could count. The test-only front shims saturated CPU 5 (harness v1), a forward-only shim helped little (v2), and the qualifying rows (v3) have the bots name their tile host themselves. See [Cluster frontier](#cluster-frontier).
- Latency and lite ran as one 14-cell campaign on the `final-stamps` and `final-lite` bundles.
- The soak failed. Its diagnosis led to two tile patches, a new bundle `final-r2`, a second soak, a labelled log diagnostic and a four-cell frontier check on `final-r2`.
- M3's jobs-4 campaign lost two seeds at startup. A jobs-1 campaign was registered and paused at 72 seeds. M3 on `final-r2` is deferred.
- The reviewer reproduced the baseline, the frontier, latency and lite. The soak, M3 and the `final-r2` rows have no reviewer run.

## Results

| Item | Result | Status | Section |
| --- | --- | --- | --- |
| Plain-09 baseline | 200 players: 200 passes twice, 225 fails twice | Reproduced | [Baseline](#plain-09-baseline) |
| Cluster frontier, final stack | 425 players: 400 and 425 pass twice, 475 fails twice; 450 mixed for the builder, failed twice for the reviewer | Reproduced | [Frontier](#cluster-frontier) |
| Ratio | 425 / 200 = 2.125x (Mac: 650 / 325 = 2.0x) | Reproduced | [Reviewer reproduction](#reviewer-reproduction-of-baseline-and-frontier) |
| Proxy latency, pinned | Every via-proxy P95 under 5 ms. Builder 4.37 ms (tick gate failed) and 4.31 ms; reviewer 4.60 and 3.20 ms qualifying, 3.97 ms unproven | Reproduced | [Proxy latency](#proxy-latency) |
| Limiter comparison | No STOP in any of six limiter rows; P95 2.81 to 6.48 ms. Inconclusive about the Mac's limiter tail | Reproduced | [Latency verdict](#latency-verdict) |
| Lite warming cost | 0.57 to 0.61% of the busiest tile's main-thread CPU (Mac: 0.38%). Combined additions unproven | Reproduced | [Lite rows](#tick-cost-lite-rows) |
| Soak on `final` | `e28-soak-r1` stopped on a collector crash at 893 s; retry `-a1` lost tiles a and b at about 544 s | Failed | [Soak on final](#30-minute-soak-on-final) |
| Soak on `final-r2` | `e28-soak-r2` lost tiles a and d to lease expiry at 1,216 s | Failed, builder run | [Soak on final-r2](#soak-on-final-r2) |
| Log diagnostic on `final-r2` | Bulk logs on tmpfs: 1,800 s, no authority stop, largest barrier 88.8 ms. Not the M4 soak row | Builder result | [Soak on final-r2](#soak-on-final-r2) |
| Frontier check on `final-r2` | 425 passes twice; 450 has one raw-P99 failure and one cell that lost a tile | Builder result | [Frontier check on final-r2](#frontier-check-on-final-r2) |
| M3 on `final` | Jobs-4 lost two seeds at startup; matrix passes all 18 cases; jobs-1 paused at 72 of 1,000 seeds, zero failures | Paused | [M3 on final](#m3-campaign-on-final) |
| M3 on `final-r2` | Not started | Deferred | [M3 on final-r2](#m3-on-final-r2) |

**Brackets.** The builder's baseline bracket is 200/225 and its frontier
bracket 425/475, with 450 mixed: one pass, one fail at 49.8 to 53.0 ms raw
P99. The reviewer's baseline bracket is also 200/225. Its frontier bracket is
425/450: both 450 repeats failed, r0 on tile d alone at 50.785 ms and r1 at
52.041 and 50.526 ms on two tiles, so 450 is a narrow fail. 425 is the highest
step with two passes in both runs. Only harness-v3 rows count toward it.

**Open items.**

- The soak line is not met on this machine. Store fsync stalls end tile
  leases. The log diagnostic points at writeback of the fixture's logs; the 600 ms a renewal
  can wait is unchanged. E29, on the branch `e29-lease-renewal`, answers a
  granted renewal without waiting for the journal barrier. Nothing from it is
  measured.
- The store's memory line. The [D12 addendum](../decisions.md#d12-addendum-the-soaks-memory-line-on-this-platform)
  judges resident plus swap from the later of minute 5 and a full receipt
  window. The soak figures in this report are minute 5 to the last sample and
  predate that ruling; the ruled figures are not computed here.
- The registration fix has no live-fire evidence.
- M3: 928 seeds on `final` are unattempted, and nothing has run on `final-r2`.
- Why the cluster's tick tail fails at 450 to 475 is open. See
  [Why the cluster fails](#why-the-cluster-fails) and
  [E31](E31-tile-cost.md). The proxy's cost is analysed in [E30](E30-proxy-cost.md).
- The combined tick additions under 5% stay unproven.
- Fixes for the audit's harness defects are in progress.

## Harness port

Status: builder checks, plus the reviewer's own live pinning check. These
are port checks, not capacity results.

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
CPU 5 busy is measured from cell start to the end of the measured minute, so
it can read lower than the minute itself (audit finding 1). The outside rule
does not count the measurement driver on Linux (finding 8).

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
suite had 19 passing tests at the port, including parsing fixtures, wrapper
arguments, private environment propagation, native main-thread selection,
recovery idempotence, partial recovery retries, bundle tampering and
compression hashes. It has 20 in the audit's later run. Initial failures and
subsequent test logs remain under `$REPO/.local/evidence/E28/harness-*`.

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
The reviewer ran its own check under a fresh name in its own drained H host
window, for example `python3 -B scripts/e28-platform-check --name repro-harness-live`.
The original record cannot be overwritten.

The Mac paths were compared by reading the diff and have not been run on a
Mac since the port. Three of them changed; see
[What changed on the Mac paths](#what-changed-on-the-mac-paths). No Rust build
or game-server load ran as part of the harness port.

## Fresh rebuild

Status: checked by the reviewer. All four Linux release bundles pass
`goliath_bundle.validate` under the explicit
[gate disposition](data/E28/gate-disposition.json). The canonical replacement
was made inside B's quiet reservation, then the reservation was released. No
capacity result is claimed by this rebuild.

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
After the bundles froze, the store's `fetch_update` was renamed to
`try_update`, so the root gates pass again. The frozen store binary was built
before that rename.
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
outside the frozen bundles. The anchor's SHA-256 at this rebuild was
`7dd3a3e222d46b319eea6cc65fc586afbe1dc33afd14c8635a0f62111bd10347`; it changed
when `final-r2` was added (see [Frozen identities](#frozen-identities)).
The removed lint-only patch is absent from both the canonical source stack and
the patch directory. The real stamp clock port remains selected.

Linux's keeper uses BOOTTIME against MONOTONIC-backed `Instant` brackets. Its
slack is `(next.after - previous.before) / 500 + 1 microsecond`, rebased after
each accepted check. The planned check (lease traces for at least 30 seconds,
increasing renewal counters, send-anchored 250 ms cadence, no false clock
anomaly or authority stop) is covered by the builder's renewal readback of the
first soak over its partial run; see [First run](#first-run-e28-soak-r1).
A real system suspend was not performed.

Disposable target directories and redundant staging copies were removed through
the measurement-aware B build window using scoped `scripts/clean-builds`, never
`--all`. This freed 10,256,855,296 apparent bytes, about 9.55 GiB. All three
manual source checkouts and the four frozen bundles remain. The workspace used
1,448,943,616 allocated bytes after cleanup, about 1.35 GiB; exact accounting is
in [the disk record](data/E28/build-disk.json).

## Frozen identities

The trust anchor is [the build manifest](data/E28/build-manifest.json), which
hashes every file in every E28 bundle, metadata and launchers included. Its
SHA-256 was `7dd3a3e222d46b319eea6cc65fc586afbe1dc33afd14c8635a0f62111bd10347`
for every measurement before `final-r2` was added, and has been
`d9cbbdcc0e3dc54e5119ccd37dd8662d9ca42a7b1e2ce33b98e50e8d08ee6698` since.
Records that store the value carry the one they ran against: the latency and
lite summary, the lite clock row and the `e28-soak-r1` and `-a1` plans carry
`7dd3a3e2...`; the `e28-soak-r2` and diagnostic plans carry `d9cbbdcc...`.
The capacity rows and M3 windows hash the bundle files directly instead.

The disabled-trace proof is `5411d3d5039c3b6dccfd9e1a3da7eb78a30eb9347474f940aae7c0fd72f917ec`.
These are frozen-file identities, separate from the decompressed raw-row
hashes in the ledgers.

| Bundle | Used executable | SHA-256 |
| --- | --- | --- |
| plain-09 | `.local/bin/goliath-e28/plain-09/horizon-client` | `f9527d1acc54c25fd98efad4f8bdaec88c9d35a6051c1fb9c286e550eceaa629` |
| plain-09 | `.local/bin/goliath-e28/plain-09/horizon-bots` | `3a4874246ad92ec7dbd2f5a8e2280b14e25df4410615e229d30fef99f2b2c9e4` |
| final | `.local/bin/goliath-e28/final/horizon-client` | `1614a5f60b87b44e1cb08c28aae6925539dc30ff5572c9fe06e723cfe3f2e9a0` |
| final | `.local/bin/goliath-e28/final/meridian` | `085414788e7bd53c49346e22ee36924940a134bc44f05336f2a43d6af282fe2c` |
| final | `.local/bin/goliath-e28/final/meridian-sidecar` | `d4523dccd4b03f2d4c6b57909f9323bcfc271ff78e43cb1ee456f5e1f23976ad` |
| final | `.local/bin/goliath-e28/final/goliath-store` | `ac3c8075a6ec92e52a0201925a2b896c4ffb2803526473630bced68cd35ba888` |
| final | `.local/bin/goliath-e28/final/e4-client` | `f886e67fbc0ac9004f7d07126465119d813b10995f22f8733a2656ef526ce8b6` |
| final-lite | `.local/bin/goliath-e28/final-lite/horizon-client` | `1370d3f34f92d6a07465f7468e9fee9c922850af5b9221f4c04b6cb9ecc19883` |
| final-lite | `.local/bin/goliath-e28/final-lite/e26_clock_probe` | `7dc09400ad28537b301528ea0b485c5247ff4cb2800e107c48693636bf6ce484` |
| final-stamps | `.local/bin/goliath-e28/final-stamps/horizon-client` | `8b285fd908948dc26cdfe8ab4baea8a064d2b215aaba26144344c797bb174088` |
| final-stamps | `.local/bin/goliath-e28/final-stamps/horizon-bots` | `73a2c53b02ecaf065942381cfdb170d74f205638a61aed3cd1ddb432c9aa4848` |
| final-r2 | `.local/bin/goliath-e28/final-r2/horizon-client` | `673fd535a946ae0b909e5ba62e897f141fccde006eb00b637058894bf06b7162` |

In `final-lite`, `final-stamps` and `final-r2`, every executable not listed
here is byte-identical to `final`'s. `final`'s own `horizon-bots` is
plain-09's. The v3 capacity cells and the soaks use the `final-stamps` bots
with tracing off (see [Direct routing](#direct-routing-and-harness-v3)).

## Plain-09 baseline

Status: reproduced by the reviewer. 200 passes and 225 fails in both runs.

The builder's first preflight rejected the plain-09 bundle with
`E28 source equality or build gates did not pass`: its build record has
`verification_status: failed` and retains `plain-09-fmt`. The reviewer then
recorded [the gate disposition](data/E28/gate-disposition.json). At the time
of these measurements its SHA-256 was
`0247ea9473e7cb4f07598f885fc0052dc3a48aaf593d7b22c6f5acb832ccfa5f`. It gained
`final-r2` entries in a later commit and is now
`e54b2b89df118d9964f8073258dd5148053de808f382e25dce4be9215042d01c`.
It accepts only the named, bundle-specific toolchain-lint failures for
`plain-09-fmt`, `root-clippy`, `proxy-fmt` and `proxy-clippy`. Tests, compile
errors, source deviations and any other failed gate still block. Plain-09 then
validated, and every measurement below used this disposition.

The builder's bracket is 200/225. Both 200-player repeats pass; both
225-player repeats are valid TPS and raw-P99 overloads. Single pilots at 150
and 250 and the 50-player smoke are retained separately and do not count
toward the staircase. The retained-data checker passes geometry, movement,
population, raw-tick and CPU/noise checks for all four staircase rows. No
rejected preflight is represented as an overload.

The reviewer's repeats agree: 200 passes twice at raw P99 46.549 and
47.882 ms, and 225 fails twice at 19.673 and 19.575 TPS. Its rows are in
`data/E28/reviewer-baseline/` and in the
[combined reproduction table](#reviewer-reproduction-of-baseline-and-frontier).

The direct single-cell Linux command now establishes the legacy baseline
quiet marker inside an existing host reservation. Both this command and the
cluster runner refuse to overwrite an existing compressed raw row. Regression
coverage includes marker setup, compressed-name protection and relative-path
compression. Unit checks do not waive release gates.

The builder's smoke command was:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-plain-50-r0'
python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e28/plain-09 --data-dir docs/experiments/data/E28/baseline --cpu-mode pin --cell 50 --repeat 0 --out docs/experiments/data/E28/baseline/e28-smoke-plain-50-r0.json
.local/bin/host-window release E28M
```

This smoke passed. Its relative-path compression failure occurred after the
measurement; the gzip was verified byte-exact, its decompressed hash was saved,
and only the duplicate uncompressed copy was removed. No measurement was rerun.
The path-handling bug is fixed and regression-tested. The completed baseline's
raw rows and retained-data check are indexed by
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

Status: reproduced by the reviewer. The builder's v3 bracket is 425/475 with
450 mixed. The reviewer's two 450 repeats both failed, which closes its
bracket at 425/450. Only harness-v3 rows count, and they ran last; the v1 and
v2 rows below are kept as evidence.

The builder's v3 staircase: both 400 and both 425 rows pass, 450 is mixed, and both 475
rows fail raw P99. This is E20's unstable-band rule: 425 is the highest
stable tested pass, not a claim that every count above it fails. All eight v3
cells are valid. CPU 4 busy ranges from 29.30% to 40.36%, and CPU 5 from
45.72% to 53.40%; bots are not late. The two 475 rows use 3.220 and 3.238
server cores and still miss the raw-P99 rule. No harness-core or bot-core
saturation is claimed as a cluster failure.

The final bundle validates under the gate disposition. The manifest and all
proxy source-stack records exclude the rejected lint-only Meridian Linux
patch. The 100-player cluster smoke passes, with all four tiles at 20 TPS, 64
pigs per tile, correct cpusets and restored slices and governors.

### Harness v1: front shims

The original staircase is harness-v1 evidence only. Both 400-player repeats
and both 425-player repeats pass, with one invalid 400/r1 startup and its
explicitly named retry retained. The last v1 row, 450/r0, passes as well. It
uses only 2.587 server cores while CPU 5 is 94.13% busy. At 425/r1 CPU 5 is
92.76% busy and servers use 2.412 cores. Continuing would have risked
measuring the test-only shims' core limit instead of the server frontier, so
no v1 row determines the frontier. The audit later found CPU 5 at 100% over
the measured minute in 400/r1-a1, 425/r1 and 450/r0 (finding 1).

The builder's cluster smoke command was:

```sh
export GOLIATH_WINDOW_OWNER=E28M
.local/bin/host-window quiet E28M 'e28-smoke-final-100-r0'
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/frontier --cpu-mode pin --counts 100 --name e28-smoke-final-100-r0
.local/bin/host-window release E28M
```

This smoke used harness-v1, whose hashes are in its raw row. The command
records that run, not a v3 reproduction. The local resume record is
`$REPO/.local/evidence/E28/measure-state.md`.

### Forward-only capacity shim and harness-v2

The reviewer asked for this change after the 450/r0 row completed. The capacity
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

CPU 5 was at 100% over the measured minute in the three v1 rows the audit
checked (400/r1-a1, 425/r1, 450/r0), so these rows under-loaded the servers
even where they read valid.

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

There are 26 retained capacity cells: 25 valid, including six valid
performance failures, and one invalid startup. Nineteen cells pass. Every
complete raw row over 1 MB is losslessly compressed. The 26 rows contain
100,281,041 decompressed bytes stored in 8,392,150 bytes, with the startup's
11,339-byte native failure evidence retained separately in 1,576 bytes.

[The complete ledger](data/E28/measurement-ledger.json) records every raw path, decompressed-byte and stored-file SHA-256, exact builder command, harness generation, outcome, CPU counters, populations, and restoration check. Every raw JSON over 1 MB is `.json.gz`; no samples were trimmed. The failed startup's native evidence is retained and hashed separately in that ledger. The first plain smoke's native exit 1 was a publication error after a passing measurement, repaired byte-exact without a rerun. The failed IO-pool startup was a readiness race: the proxy binds TCP before starting its pool, and the old launcher returned on TCP connect. The runner now waits at most 10 seconds for the complete pool message and rejects early proxy exit. Its failed row and explicit `-a1` retry remain separate.

Retained-data checks reproduce the baseline bracket and v3 endpoint arithmetic:

```sh
python3 -B scripts/e17-flat09-report --data-dir docs/experiments/data/E28/baseline
python3 -B scripts/e20-results e28-frontier-v3 --baseline 200 --data-dir docs/experiments/data/E28/frontier
```

The reviewer ran fresh cells from the repository root after validating both
selected bundles and checking the disabled-trace guard, which compiles a tiny
native probe through the build reservation before any measurement window.
As run by the reviewer:

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

Each cell stays in the foreground. Keep invalid and failed reproductions and
do not repeat a mixed step. If the reproduced baseline differs, supply that
value to readback rather than reusing 200. After an interrupted cell anywhere
in this report, restore the host as described under
[Harness port](#harness-port) before removing a stale host lock.

### Verification

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

Three harness failures were resolved without hiding results: the initial
single-cell marker gap, relative-path compression after the passing plain
smoke, and the IO-pool readiness race before the invalid cluster startup.
The native disabled-trace probe first had a test-module nesting error; the
unchanged method was put in its original module context and the guard tests
then passed. No application or frozen binary changed for that fix.

### Reviewer reproduction of baseline and frontier

The reviewer ran every baseline and v3 frontier step again, two fresh repeats each, with the builder's documented commands, the same bundles and `--cpu-mode pin`. All 12 cells are valid. Data: `data/E28/reviewer-baseline/` and `data/E28/reviewer-frontier/`.

| Step | Repeat 0 | Repeat 1 | Server cores | Proxy cores |
| --- | --- | --- | --- | --- |
| Plain 200 | pass, 19.99 TPS | pass, 19.99 TPS | | |
| Plain 225 | fail, 19.67 TPS | fail, 19.57 TPS | | |
| Cluster 400 | pass | pass | 2.46, 2.41 | 0.64, 0.65 |
| Cluster 425 | pass | pass | 2.64, 2.67 | 0.69, 0.72 |
| Cluster 450 | fail | fail | 2.97, 2.94 | 0.79, 0.79 |
| Cluster 475 | fail | fail | 3.29, 3.30 | 0.87, 0.88 |

The baseline reproduces at 200 and the frontier at 425, so the ratio is
2.125x on this machine. The reviewer's 450 step failed both repeats where the
builder's was mixed, which closes the bracket one step lower. The Mac's ratio
was 2.0x (650 / 325).

### Why the cluster fails

The cluster fails on raw tick P99, not on TPS. The "Server cores" column sums
the CPU of the registered server processes. Under pinning the budget is the
four CPUs themselves, and they were busier than that sum:

| Cell | Server cores | CPUs 0 to 3 busy, measured minute | Tile main threads, cores each |
| --- | ---: | ---: | --- |
| `e28-frontier-v3-425-r0` | 2.668 | 2.95 (73.7% each) | 0.39 to 0.41 |
| `repro-e28-frontier-v3-450-r0` | 2.972 | 3.27 (81.8%) | 0.43 to 0.45 |
| `e28-frontier-v3-475-r0` | 3.220 | 3.53 (88.2%) | 0.46 to 0.49 |
| `repro-e28-frontier-v3-475-r1` | 3.296 | not computed | 0.49 to 0.50 |

The figures beyond "Server cores" are the audit's, recomputed from the raw
rows. At the failing 450 and 475 steps each tile's main thread averages 0.43
to 0.50 core; the plain server's main thread passes 200 players at 0.82 to
0.84 core. At 475 the server CPUs are about 88% busy and about 0.3 core on
them belongs to no registered process. Interrupt and softirq time for
loopback traffic and kernel threads are the likely source; none is
attributed. The four tiles themselves use about 2.4 cores. At 475 the raw tick
P50 is 29 to 33 ms while the main thread's CPU is about 24 ms per tick, so the
main thread spends part of each tick not running. It was either waiting for
one of four CPUs that were 88% busy, or blocked on its own worker threads. No
run-delay data was kept, so the cause of the tick tail is open.
[E31](E31-tile-cost.md) takes the question further from retained data. An
earlier version of this report said each tile's main thread "runs out of room
first"; the data does not support that, and the statement is withdrawn.

## Proxy latency

Status: reproduced by the reviewer. Every pinned via-proxy P95 is under 5 ms,
and the reviewer has two qualifying pinned rows. The limiter comparison is
inconclusive.

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
The configured window is 60 seconds after 30 seconds warmup, followed by two
seconds of decode drain. No forwarding tap, handshake shim or login relay
is present. Nothing subtracts STOP pauses or direct-reference quantiles.
This covers selected interior movement, not crossing latency or all packets.

Two via rows per CPU mode were run. The comparison mode keeps servers
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

### Builder run

The required reviewer `475/r1` result existed and the host window was free
before the first builder cell. All 14 cells in this latency/lite campaign
ran serially in the foreground under `E28L`. The builder ran no other work
during a cell. [The complete ledger](data/E28/latency-lite-ledger.json)
retains exact portable commands, outcomes, decompressed and stored hashes,
and each cell's before/after restoration policy. `python3` in that ledger
replaces only the private interpreter path, not measurement data.

Cell names below have the common prefix `e28-latency-`. Times are nearest-rank
milliseconds. The failed cell has no qualified timing readback, not zero
latency. Every complete row has zero unmatched or ambiguous sends.

| Cell | P50 | P95 | P99 | Worst tile P95 | Matched / sent | Proxy cores | Threads | Disposition |
| --- | ---: | ---: | ---: | --- | ---: | ---: | --- | --- |
| [pin-direct-r0](data/E28/latency/e28-latency-pin-direct-r0-summary.json.gz) | - | - | - | - | - | - | - | collector failed; retained |
| [pin-direct-r0-a1](data/E28/latency/e28-latency-pin-direct-r0-a1-summary.json.gz) | 0.048226 | 1.168883 | 2.822854 | a, 1.610128 | 9,848 / 9,848 | 0.694676 | 856..861 | valid reference |
| [pin-via-r0](data/E28/latency/e28-latency-pin-via-r0-summary.json.gz) | 0.121056 | 4.371403 | 10.112082 | c, 5.127457 | 9,864 / 9,864 | 0.745214 | 872..877 | raw-tick failure; not qualifying |
| [limiter-via-r0](data/E28/latency/e28-latency-limiter-via-r0-summary.json.gz) | 0.313205 | 6.479126 | 12.038556 | a, 7.404990 | 9,640 / 9,640 | 0.768256 | 871..876 | valid; latency line failed |
| [limiter-direct-r0](data/E28/latency/e28-latency-limiter-direct-r0-summary.json.gz) | 0.135669 | 1.774439 | 3.390582 | b, 2.249587 | 9,638 / 9,638 | 0.738986 | 856..861 | valid reference |
| [limiter-via-r1](data/E28/latency/e28-latency-limiter-via-r1-summary.json.gz) | 0.123545 | 2.814577 | 6.141405 | a, 3.841595 | 9,694 / 9,694 | 0.784902 | 872..876 | valid; overall line passed |
| [pin-via-r1](data/E28/latency/e28-latency-pin-via-r1-summary.json.gz) | 0.579682 | 4.313780 | 9.298967 | d, 5.839312 | 9,666 / 9,666 | 0.716178 | 871..877 | valid; overall line passed |

The original direct cell failed after packet collection because Linux's
frozen authority status explicitly returns `clock: null`. The collector
attempted `.get()` on null. Its row and native traces remain unchanged and
failed. The collector now accepts the unavailable telemetry field while
still requiring authorized status and all four complete native authority/
clock trace audits. A regression test covers null, lost authorization and
nonzero closure counters. The [explicit additional-attempt record](data/E28/latency-lite-attempts.json)
registered `-a1` before its run. No performance or intrusion failure was retried.

`pin-via-r0` retained 20 TPS and zero dropped ticks but raw P99 was
51.789515 / 50.573152 / 51.158516 / 47.079673 ms on a / b / c / d.
It cannot qualify under the unchanged fixture gates. CPU 5 was 94.68%
busy over the whole cell, below the 95% invalidation line. Over the measured
minute alone it was 97.66% busy, so this cell's harness core was saturated
while it was measured (audit finding 1); the rule now applies to the measured
minute too. Its outside-process limits passed. It was not reclassified as
noise or replaced. No causal attribution of this tick failure to outside work
is established.

| Complete cell | Tile a P95 | Tile b P95 | Tile c P95 | Tile d P95 | Retained send window, seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| pin-direct-r0-a1 | 1.610128 | 0.967399 | 0.637388 | 1.314480 | 61.568030 |
| pin-via-r0 | 3.453428 | 4.295749 | 5.127457 | 4.608811 | 61.646463 |
| limiter-via-r0 | 7.404990 | 3.721255 | 6.533829 | 7.162697 | 60.253078 |
| limiter-direct-r0 | 0.793233 | 2.249587 | 1.860129 | 1.942759 | 60.217413 |
| limiter-via-r1 | 3.841595 | 2.689268 | 2.929898 | 1.906388 | 60.567814 |
| pin-via-r1 | 4.537922 | 3.807372 | 3.460687 | 5.839312 | 60.431627 |

These are the unchanged E25 counter/census clock brackets around the
configured 60-second phase, not an assertion that every retained bracket
lasts exactly 60 seconds. All sends inside the actual recorded brackets
and the fixed two-second decode drain are retained. Nothing was trimmed
or rescaled to obtain an exact duration. Each tile has two witnesses.
Complete rows retain all 425 sessions, 42 readers and 64 pigs per tile at
both census endpoints. The per-session ledgers distinguish 425 proxied
sessions from the references' 417 proxied plus eight signed direct sessions.
Both paths retain the same seeded routes, compression threshold 256 and
no encryption. Census vectors and every selected send remain in the raw rows.

#### STOP association, builder rows

No complete latency cell issued a server or bot STOP, including both
limiter via rows. Both caps remained enabled and their native aggregate
process-CPU readers reported no error. Shutdown CONT calls are retained;
they are not fabricated STOP intervals. The overlap readbacks contain no
open STOPs, dropped signal events or unknown matched spans.

| Cell | P95-tail sends | Server span overlap | Bot span overlap |
| --- | ---: | ---: | ---: |
| pin-direct-r0-a1 | 493 | 0 / 493 | 0 / 493 |
| pin-via-r0 | 494 | 0 / 494 | 0 / 494 |
| limiter-via-r0 | 483 | 0 / 483 | 0 / 483 |
| limiter-direct-r0 | 482 | 0 / 482 | 0 / 482 |
| limiter-via-r1 | 485 | 0 / 485 | 0 / 485 |
| pin-via-r1 | 484 | 0 / 484 | 0 / 484 |

All shares are 0%. These are E25's signal-call bracket associations, not
kernel stop acknowledgements or a causal subtraction. Both builder pin P95
values are below 5 ms, but only the second row passes every fixture gate, so
the builder's pin pair is not two qualifying M4 observations. The limiter pair
is mixed on the overall latency line. The direct references do not repair
either failure, and their quantiles are not subtracted.

### Reviewer reproduction of latency

The reviewer reran the fixed order with fresh `repro-e28-latency-*` names, the same plan, bundles and runner. Data: `data/E28/reviewer-latency/`. Nearest-rank milliseconds; every complete row matched all selected sends.

| Cell | Status | P95 | P99 | Tail sends overlapping a STOP |
| --- | --- | ---: | ---: | --- |
| pin-direct-r0 | valid reference | 1.21 | 2.78 | 0 / 484 |
| pin-via-r0 | valid, passes | 4.60 | 9.04 | 0 / 489 |
| limiter-via-r0 | valid, passes | 3.86 | 9.25 | 0 / 487 |
| limiter-direct-r0 | valid reference | 1.46 | 2.57 | 0 / 483 |
| limiter-via-r1 | valid, passes | 3.42 | 7.39 | 0 / 483 |
| pin-via-r1 | unproven: authority trace truncated | (3.97) | (9.64) | 0 / 482 |
| pin-via-r1-a1 | valid, passes | 3.20 | 7.05 | 0 / 483 |

`pin-via-r1` is kept as failed. Tile b's lease trace ends with a partial line and no newline because the tile stopped before flushing, and the strict collector refuses a row whose authority trace can't be decoded in full. Its timing is shown in brackets and isn't counted. That is a collection fault, not a latency or tick result, so the reviewer ran one explicitly named retry, `-a1`, under the same rule the builder used.

As run by the reviewer from the repository root; `pin-via-r1-a1` was
registered and run on its own after this loop. The runner acquires and
releases each window under the exported owner, so do not wrap a second owner
around it.

```sh
export GOLIATH_WINDOW_OWNER=E28R
for e28_spec in pin-direct-r0 pin-via-r0 limiter-via-r0 limiter-direct-r0 limiter-via-r1 pin-via-r1; do
  e28_mode="${e28_spec%%-*}"
  e28_direct=()
  case "$e28_spec" in *-direct-*) e28_direct=(--direct-reference);; esac
  python3 -u -B scripts/e25-latency-cell-r2 --name "repro-e28-latency-${e28_spec}" --bins .local/bin/goliath-e28/final-stamps --data-dir docs/experiments/data/E28/reviewer-latency --players 425 --cpu-mode "$e28_mode" --plan docs/experiments/data/E28/latency/e28-latency-plan-r1.json --stop-brackets "${e28_direct[@]}"
  e28_summary="docs/experiments/data/E28/reviewer-latency/repro-e28-latency-${e28_spec}-summary.json"
  test -f "$e28_summary" || e28_summary="${e28_summary}.gz"
  python3 -B scripts/e25-stop-overlap --summary "$e28_summary" --data-dir docs/experiments/data/E28/reviewer-latency --name "repro-e28-latency-${e28_spec}-overlap"
done
```

If a summary is compressed, pass its actual `.json.gz` path to overlap
readback. Keep failed or invalid rows; no loop implies permission to overwrite
them or replace them with another repeat.

### Latency verdict

Verdict on this platform: every pinned via-proxy P95 measured here is under 5 ms (builder 4.37 and 4.31, reviewer 4.60, 3.97 unproven, and 3.20), and the reviewer has two qualifying pinned rows, 4.60 and 3.20 ms. The builder's 4.37 ms row stays failed on its tick gate, and its harness core was saturated over the measured minute. So, at this machine's 425-player frontier and under D12 pinning, the proxy path is under the 5 ms line. The comparison mode issued no STOP in any of its six rows across both runs, builder and reviewer, because the servers stayed below their four-core cap at 425 players. This machine therefore can't show that pinning removes the Mac's limiter tail. Its limiter rows ranged from 2.81 to 6.48 ms with no pauses at all. The Mac result under D9 stays as it was.

The audit adds that no test or cell shows the Linux limiter issuing a STOP at
all. In all six limiter rows the server budget's largest debt was at most
9.9 ms of CPU against a 20 ms threshold, and the bot budget's at most 0.7 ms
against 5 ms, all before the measured window. The limiter ran and was never
close to pausing anything.

## Tick-cost lite rows

Status: reproduced by the reviewer. Warming costs about 0.6% of the busiest
tile's main-thread CPU. The builder's two B rows fall outside the A controls'
spread and the reviewer's two fall inside it. The combined bound stays
unproven.

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

### Lite results and declared charges

The fresh [clock calibration](data/E28/lite/e28-lite-clock-r0.json) uses probe
SHA-256 `7dc09400ad28537b301528ea0b485c5247ff4cb2800e107c48693636bf6ce484`.
All 32 batches of 100,000 reads completed. Maximum batch CPU cost is
621.95276 ns per read; reported resolution is 1 ns. The allowance is
`ceil(621.95276) + 1 = 623 ns`. The observed minimum positive step is 66 ns,
step GCD is 1 ns, and none of 100,000 resolution samples has a zero step.
This is E26b's empirical conservative allowance, not a global worst-case
clock-latency bound. Bundle identity, noise and restoration checks pass.

All six rows pass host validity and the original TPS/raw-tick gates, with
zero raw-tick drops. Names below have prefix `e28-lite-`. Each selected tile
comes from the last five warmup seconds and is not reselected afterward.

| Cell | Arm | Selected tile | Main cores | Selected raw P50 ms | Selected raw P99 ms |
| --- | --- | --- | ---: | ---: | ---: |
| [a1](data/E28/lite/e28-lite-a1.json.gz) | A | b | 0.395927 | 21.633467 | 37.497985 |
| [b1](data/E28/lite/e28-lite-b1.json.gz) | B | a | 0.413878 | 24.891179 | 38.067368 |
| [a2](data/E28/lite/e28-lite-a2.json.gz) | A | b | 0.398304 | 21.241104 | 40.778922 |
| [a3](data/E28/lite/e28-lite-a3.json.gz) | A | b | 0.392896 | 20.860476 | 32.379102 |
| [b2](data/E28/lite/e28-lite-b2.json.gz) | B | b | 0.398126 | 20.414773 | 35.006268 |
| [a4](data/E28/lite/e28-lite-a4.json.gz) | A | b | 0.394295 | 21.369375 | 38.905329 |

B1's selected tile is a, while every A control selected b, so B1's intrusion
check compares different tiles.

The uncharged warming share is the ratio of CPU sums over complete
query-bracketed cycles, not a mean of per-cycle percentages.

| B row | Cycles | Cycle CPU s | Warming CPU s | Warm/cycle | Warm ms/cycle | Per-cycle share P50 / P99 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| b1 | 1,227 | 25.395428989 | 0.144429364 | 0.568722% | 0.117709 | 0.574525% / 1.168950% |
| b2 | 1,220 | 24.283829331 | 0.145141787 | 0.597689% | 0.118969 | 0.614080% / 0.974652% |

| B row | Reads charged | Read charge s | Positive main-CPU delta | Delta charge s | Charged lower bound | Intrusion |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| [b1](data/E28/lite/e28-lite-b1-summary.json) | 57,970 | 0.036115310 | 0.018522567 core | 1.136415187 | 0% | main CPU and P50 outside |
| [b2](data/E28/lite/e28-lite-b2-summary.json) | 62,198 | 0.038749354 | 0.002770457 core | 0.169037870 | 0% | P50 outside |

The A main-CPU mean is 0.395355794 core and its range is 0.005407781 core.
The A P50 mean/range is 21.2761055 / 0.772991 ms; P99 is
37.3903345 / 8.399820 ms. B1 shifts by +0.018522567 core,
+3.6150735 ms P50 and +0.6770335 ms P99. B2 shifts by +0.002770457
core, -0.8613325 ms P50 and -2.3840665 ms P99. E26b tests absolute
shifts, so B2's faster P50 still fails. No rule was relaxed and no
out-of-spread row was replaced.

After both declared charges each lower bound clamps to zero. That does
not mean warming costs zero and cannot support an under-5% upper bound.
Both rows are unproven under the intrusion rule. The recorder covers warming
only, so the combined additions remain unbounded by these lite observations.

### D12 fields for the latency and lite cells

Every cell below passed the D12 host checks as coded, including the
collector-failed reference and the raw-tick-failed via row. CPU percentages
cover cell start through steady end, as in the capacity port. Outside totals
and maxima use steady / whole-cell order. The short clock row uses its native
probe phase. By the rule as coded no CPU 5 saturation or outside-process
invalidation occurred, so no noise retry was invoked. Over the measured
minute alone, `latency-pin-via-r0` had CPU 5 at 97.66% (see
[Builder run](#builder-run)). The one `-a1` attempt fixes the separately
recorded null telemetry collector failure.

| Cell | Start load | CPU 4 busy | CPU 5 busy | Outside total cores, steady / whole | Largest outside process cores, steady / whole | Swap-in pages |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| latency-pin-direct-r0 | 3.968 | 34.99% | 52.93% | 0.259 / 0.229 | 0.170 / 0.156 | 18 |
| latency-pin-direct-r0-a1 | 3.976 | 35.07% | 56.33% | 0.344 / 0.281 | 0.171 / 0.154 | 136 |
| latency-pin-via-r0 | 3.857 | 38.56% | 94.68% | 0.662 / 0.619 | 0.238 / 0.248 | 8,513 |
| latency-limiter-via-r0 | 3.840 | 65.11% | 74.23% | 0.487 / 0.387 | 0.197 / 0.198 | 1,281 |
| latency-limiter-direct-r0 | 3.782 | 64.58% | 67.62% | 0.294 / 0.310 | 0.180 / 0.187 | 13,306 |
| latency-limiter-via-r1 | 3.877 | 65.72% | 72.29% | 0.403 / 0.348 | 0.189 / 0.172 | 816 |
| latency-pin-via-r1 | 3.715 | 35.18% | 54.26% | 0.153 / 0.190 | 0.042 / 0.068 | 466 |
| lite-clock-r0 | 3.943 | 0.00% | 7.44% | 0.019 / 0.019 | 0.010 / 0.009 | 0 |
| lite-a1 | 3.991 | 33.60% | 57.45% | 0.220 / 0.255 | 0.101 / 0.129 | 262 |
| lite-b1 | 3.825 | 32.94% | 45.05% | 0.136 / 0.159 | 0.043 / 0.067 | 708 |
| lite-a2 | 3.874 | 31.84% | 44.88% | 0.139 / 0.133 | 0.065 / 0.062 | 80 |
| lite-a3 | 3.775 | 31.49% | 41.06% | 0.092 / 0.098 | 0.032 / 0.042 | 3 |
| lite-b2 | 3.848 | 31.46% | 38.11% | 0.079 / 0.076 | 0.020 / 0.019 | 6 |
| lite-a4 | 3.765 | 31.28% | 43.71% | 0.110 / 0.106 | 0.041 / 0.040 | 6 |

Names in this table have prefix `e28-`. Every row has throttle-counter
coverage with zero increases, the performance governor during the window,
turbo unchanged, correct server/bot/harness cpusets, and exact restoration
of the original slice and governor policies. Pin uses `0-3 / 4 / 5`;
limiter uses `0-4 / 0-4 / 5`. No outside process exceeds 0.5 core in either
required phase. Outside totals are reported, not substituted for the
per-process validity rule.

[The numerical readback](data/E28/latency-lite-summary.json) retains all six
CPUs' busy/total ticks, every core's observed frequency range, first/last
throttle counters, available-memory endpoints, swap-in, cpuset placement,
outside totals/maxima and per-tile populations for each game row. Its clock
record retains the same native snapshots for the calibration. The linked
raw rows retain every frequency sample and outside counter. These fields
are not replaced with invented Linux equivalents of Darwin counters.

There are 14 retained cells: seven latency attempts, one calibration and
six lite/control rows. Thirteen complete, one collector fails. One complete
via row fails raw ticks; one valid limiter row fails the latency line;
both lite B readbacks fail intrusion. These outcomes are distinct from
host-noise invalidation. No tails, invalid attempts or failed observations
were removed. Native traces remain under `$REPO/.local/evidence/E28/`.
All run/world directories were deleted after extraction.

[The evidence index](data/E28/latency-lite-evidence-index.json) identifies
110 artifacts with decompressed-byte and stored-file SHA-256 values.
It contains 154,369,535 decompressed bytes in 13,384,746 stored bytes.
Every JSON over 1 MB is losslessly compressed. Publication redacts outside
names and private paths. [Final verification](data/E28/latency-lite-verification.json)
checks all row/reference hashes, native authority hashes, unchanged selected
bundles and every restoration. The seven requested suites register 114 tests:
108 pass, six skip unavailable retained Mac artifacts, none fail. No Cargo
command or measured-binary rebuild ran. The existing direct-bot suite compiles
only its tiny native disabled-trace guard through its build reservation.

`host-window build E28L -- scripts/clean-builds` completed after verification,
freed 0 MB and left the 113 MB target below its cap. The project occupied
1,631 MB at cleanup. No run or world folder remained.

### Reviewer reproduction of lite rows

The reviewer reran the calibration, four A controls and two B rows with fresh `repro-e28-lite-*` names. Data: `data/E28/reviewer-lite/`. Control `a4` failed in the harness, not the measurement: `goliath-cluster` hit an undecodable `E10_PHASE` log line. It is kept, and one explicitly named retry, `a4-a1`, replaced it as the fourth control.

| B row | Warm/cycle (ratio of sums) | Per-cycle P50 / P99 | Inside A spread | Charged lower bound |
| --- | ---: | --- | --- | ---: |
| b1 | 0.579% | 0.589% / 1.068% | yes | 0% |
| b2 | 0.606% | 0.616% / 1.246% | yes | 0.446% |

Both reproduced B rows pass the intrusion check, where the builder's two did not. Taken with the builder's 0.569% and 0.598%, speculative warming on the final stack costs about 0.6% of the busiest tile's main-thread CPU on this machine, against 0.38% on the Mac, and is far below 5%. As on the Mac, the lite recorder covers warming only, so the combined additions stay unproven.

As run by the reviewer, in the preregistered order (the `a4-a1` retry was
registered and run on its own):

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

## 30-minute soak on final

Status: failed on this machine. `e28-soak-r1` stopped on a collector crash at
893 s. Its registered retry `e28-soak-r1-a1` lost tiles a and b at about
544 s. Neither reached minute 30. The diagnosis is the builder's; the reviewer
took its two bugs as the basis for the fixes in `final-r2`
([Fixes](#fixes-for-the-soak-failure)). The soak on `final-r2` is under
[Measurements on final-r2](#measurements-on-final-r2).

### Protocol and log integrity

The [soak and M3 protocol](data/E28/soak-m3-protocol.json) was registered before
measurement. The reviewer's lite B2 summary existed and the host window was
free before acquisition under `E28S`.

The run is `e28-soak-r1`, with final servers, 425 players, pinning and direct
final-stamps bots with tracing disabled. D9's 10% crossing cohort has 42
players here, so the unchanged cargo fixture gives 42 unique items, not the
Mac's 65. RSS, owner audits, cargo conservation, crossings per minute, TPS,
authority events and long-window raw tick P99 are retained separately.
Whole-window D12 evidence includes startup, the shortened loaded interval,
checkpoint/recovery and cleanup. Linux clock histograms are unavailable;
native lease traces check increasing renewal counters and send-anchored
250 ms planned wakes without pretending to supply Darwin telemetry.

Before measurement, the reviewer reported an EOF-truncated lease record and
a malformed live phase record in its latency/lite reproductions. The
[log-integrity port](data/E28/soak-m3-log-integrity-port.json) records the
E28-only response. Proxy stdout gets its own file, leaving native stderr
as the sole phase-log writer. Tile shutdown keeps console `stop` and the
180-second kill fallback, but drains the stdout reader before closing its
log. Every fallback is recorded. Lease writers remain detached in the frozen
binary. A partial tail or malformed complete record makes strict authority
evidence unproven, never a zero-event pass. Live phase tails get at most one
second to complete, with framing snapshots retained; malformed complete
records fail and retain their native bytes. No prior reviewer failure is
repaired or reclassified. No frozen executable changes for this port.

### First run: e28-soak-r1

The [failed-run readback](data/E28/soak/e28-soak-r1-result.json) retains the
raw row, all RSS samples, native authority traces and partial ticks. Collection
stopped with `ProcessLookupError(3, 'No such process')`, 893.391 seconds after
the steady-start timestamp. The last complete RSS sample is at 852.348 seconds,
not minute 30. Fourteen ownership audits have zero violations and zero
generation regressions, with three crossing-during-query brackets retained
as unresolved. All 42 unique cargo items were conserved after the shortened
run's clean all-tile checkpoint and recovery. This is not 30-minute safety
evidence.

The failure-boundary process counters contain every tile, proxy, store,
sidecar and bot group. Bots remained online, with no reported late rounds or
failures, and all five bot lifetimes were 3,300 seconds. Source inspection
found that `/proc` worker reads handled `ENOENT` but not `ESRCH` when a listed
thread exited. A regression now handles retired-worker `ESRCH` while keeping
main-thread disappearance fatal and retaining previously observed CPU.
The exact failing TID and stack were not captured, so that explanation is
supported but not proved. The failed raw row and its original counter hash
are unchanged.

Fourteen complete minute buckets each have 375 to 381 activations. The final
partial bucket has 336. Native owner-status TPS samples span 19.988 to 20.032
on a, 19.973 to 20.021 on b, 19.960 to 20.033 on c and 19.938 to 20.051 on d.
There is no final game-time endpoint TPS measurement. All 17,925 retained
phase records parse with complete framing under the split-stream adapter.

The table compares the actual minute-5 sample at 300.015 seconds with the last
retained sample at 852.348 seconds. Its right column is not a minute-30 result.

| Process | Minute 5 MiB | Last retained MiB | Observed change |
| --- | ---: | ---: | ---: |
| Proxy | 314.320 | 314.164 | -0.05% |
| Store | 33.121 | 38.273 | +15.56% |
| Tile a | 327.160 | 330.816 | +1.12% |
| Tile b | 266.289 | 270.457 | +1.57% |
| Tile c | 280.215 | 278.938 | -0.46% |
| Tile d | 259.262 | 263.949 | +1.81% |
| Sidecar 0 | 3.172 | 3.199 | +0.86% |
| Sidecar 1 | 3.176 | 3.207 | +0.98% |
| Sidecar 2 | 3.184 | 3.207 | +0.74% |
| Sidecar 3 | 3.211 | 3.211 | 0.00% |

Store RSS already exceeds +10% at the shortened endpoint. The receipt window
first filled at loaded second 334.224, 34.287 seconds after the minute-5
sample. Capacity stays 9,216, snapshot capacity stays 16 MiB after minute 5,
the 72/s planning bound is not exceeded, and the retention/singleton checks
pass. No leak cause or final minute-30 growth is established. Neither the
baseline sample nor the memory threshold was moved.
[Every-process minute RSS](data/E28/soak/e28-soak-r1-every-process-minute-rss.json)
includes the collection helpers and both drivers, with missing minutes
explicit. The original production-process judgment matches E24/E25; helper
RSS is reported separately, including its evidence-buffer growth. All
per-second observations remain in the raw and audit rows.

Long-window raw tick P99 remains separate from the soak pass line. Only the
17,067 ticks per tile retained before failure are available, with zero drops
and no overlapping sequences. Their P99 is a 36.499, b 39.876, c 35.002 and
d 38.232 ms. No 30-minute quantile is reported or inferred from them.

All four original and four recovery lease logs have complete framing, with
zero observed authority-stop or clock-anomaly records. The
[explicit partial renewal readback](data/E28/soak/e28-soak-r1-renewal-readback-r2.json)
checks increasing counters, original-send deadlines and 15,312 planned wakes
anchored exactly 250 ms after their preceding original send. Observed send
gaps range from 250.066 to 261.491 ms. The original processes' installed
counters reach 3,835 / 3,831 / 3,827 / 3,823, including startup and shutdown.
Eight calls return `Err(WouldBlock)` and later renewals succeed. Those errors
are retained, not mistaken for missing successful grant installs. Every
successful loaded grant has its install record; unknown or missing replies,
malformed framing and authority events still fail. The earlier readback's
stricter all-calls-installed field is retained beside this corrected ledger.
This verifies the partial BOOTTIME run only. There is no 30-minute clock
qualification and no real suspend test.

The whole host window is 1,077.217 seconds. CPU 5 is 63.16% busy, outside CPU
totals 0.11756 core and the largest outside process averages 0.04116 core.
No throttle counter rises; 8,214 swap-in pages are recorded. D12 placement,
start-load and noise checks pass. Slice, governor and turbo policies restore
exactly. These fields cover startup through recovery/cleanup, not just the
last successful RSS sample. They do not repair the collection failure.

### Retry: e28-soak-r1-a1

The reviewer authorized one registered retry, `e28-soak-r1-a1`, for the
collection crash. Its [retry registration](data/E28/soak-retry-plan-r1-a1.json)
preserves the fixture and the pass line, and `e28-soak-r1` stays failed.

The retry also failed, at about 544 seconds
of bot runtime. The [bot-exit diagnosis](data/E28/soak/e28-soak-r1-a1-bot-exit-diagnosis.json)
checks actual recorded argv, archived final telemetry, stderr, native process
exit statuses and the backend logs. All five groups have exactly one
`--seconds 3300`, a 55-minute lifetime, plus their direct handshake host.
The helper appends host routing without dropping duration. There is no route
completion limit explaining this exit.

The generic failure message contained an earlier cached A sample with
`failed: 0`. Its final archived sample has 96 failures and exit 1, with stderr
`horizon-bots: not all bots remained online`. B also exits 1 with 96 failures;
the crossing group exits 1 with 42. C and D remain online at failure capture.
Tile A itself exits 1 after a native panic during new-player registration,
`Fault(AlreadyExists)`. Tile B separately exhausts its confirmed lease after
two WouldBlock reads and an overall-deadline timeout, recording
`renew-keeper-deadline`. No clock-anomaly event is observed. Its refused clean
shutdown cut is retained. These are real cluster failures, not a harness timer.

The retry's whole-window D12 checks pass: CPU 5 is 74.45% busy, the largest
outside process averages 0.15894 core, no throttle counter rises and policy
restoration is exact. The last RSS sample is at 485.153 seconds, with seven
completed owner audits. There is no minute-30 endpoint or 30-minute pass.
The reviewer required a stop and a report if the cluster actually dropped a
group, so no `a2` was registered and the next M3 window was held.

### Diagnosis

Status: builder diagnosis. Fixes 1 and 2 below were built afterwards as
`final-r2`; see [Fixes for the soak failure](#fixes-for-the-soak-failure).
The numbers are in the [diagnosis record](data/E28/soak/e28-soak-diagnosis-r1.json).

`e28-soak-r1-a1` lost two tiles. One cause led to both losses, through two
separate weaknesses. A slow durable barrier in the store made tile b's lease
keeper give up on renewals the store had already granted. Tile b then
dropped its players, as designed. The proxy's fallback sent one of b's
crossers to tile a as a fresh login. Tile a tried to register a player the
store already held and panicked when the store refused.

**Tile b.** Times are on tile b's keeper clock. Store times are 2.656 s
later on the store's clock.

| Renewal | Sent | Store queue | Barrier | Store reply would reach b | Keeper saw |
| --- | ---: | ---: | ---: | ---: | --- |
| 2191 | 548.591 | 0.0 ms | 10.7 ms | 548.602 | Granted, deadline 549.491 |
| 2192 | 548.841 | 0.0 ms | 409.5 ms | 549.250 | `WouldBlock` at 549.043 |
| 2193 | 549.092 | 157.9 ms | 167.5 ms | 549.420 | `WouldBlock` at 549.298 |
| 2194 | 549.343 | 67.4 ms | 32.9 ms | 549.454 | Overall deadline at 549.450 |

The keeper stopped at 549.491 with `renew-keeper-deadline`. The proxy logged
b's shutdown kicks at the same instant. The store committed all three late
renewals. The keeper sent on time: 2192 reached the store 0.07 ms after its
send. Each reply was abandoned at the renewal client's 200 ms per-phase read
cap (`lease.rs:215` in the patched tile source, applied by
`crates/goliath-store/src/client.rs:81-84`). The schedule in
`crates/goliath-model/src/lease.rs` already allows each call to run until its
call deadline, 850 ms after the last granted send here. 2192's reply would
have arrived at 549.250, before that deadline, and moved b's deadline to
549.741. That counterfactual is computed, not measured.

The 409.5 ms is time inside fsync. The batch held one command and queued for
0.01 ms. CPUs 0-3 were 73-88% busy in the one-second samples from 548 to
552 s, so they were not saturated at that resolution. No schedstat was
retained, so runnable delay of the store and keeper threads is unobserved.
The slow barriers follow a 31.2 s cycle. In a1, 26 of the 37 barriers over
50 ms after second 150 fall in one 2.5 s slice of that cycle, which covers 8%
of the time. Every cycle produced a slow barrier: 59 to 409 ms. CPU bursts
of the volume's unconfined kernel threads land on the same cycle, at 520.7
and 552.3 s among others. r1 shows the same spacing. The kernel writes back a
dirty file about 30 s after it is first dirtied (`dirty_expire_centisecs`
3000, flusher every 5 s). The fixture keeps rewriting large logs on the
store's volume. Each tile wrote about 240 MB of `anticheat.log` (bot
prediction flags, one line per tick) and 150 MB of stderr in under ten
minutes. The Mac ran the same default-on anticheat log, according to E24's
evidence notes.

Barrier quantiles match across both Linux attempts: p50 12.4 ms, p99 32.8 ms,
p99.9 97 to 98 ms. r1 had a 286 ms maximum and eight `WouldBlock` renewals
and survived. a1 had 409 ms and did not. The Mac's retained barrier maxima
were 41.2 ms (E24 diagnosis-r1) and 19.8 ms (heap-control-200-r3), and its R2
soak had zero authority stops in 30 minutes. E24's one comparable Mac loss
had the same shape: a 435 ms barrier during a limiter STOP, `WouldBlock`,
then keeper expiry. Cadence r2 fixed the scheduling and kept the 200 ms read
cap.

The diagnostic `e28-diag-fsync-r1` tested the writeback mechanism without any
cluster binary (`scripts/e28-diag-fsync-probe`). A 25 Hz 4 KiB append and
fsync ran on the data volume for 100 s per case. Eight unsynced writers
appended 360 kB/s each, about the a1 log rate.

| Case, after 35 s | p50 | p99 | p99.9 | Max |
| --- | ---: | ---: | ---: | ---: |
| No writers | 7.70 ms | 11.63 ms | 15.52 ms | 15.60 ms |
| Writers on the data volume | 9.58 ms | 14.26 ms | 39.75 ms | 42.48 ms |
| Writers on tmpfs | 7.72 ms | 12.10 ms | 19.24 ms | 21.30 ms |

With writers on the data volume, writeback came in bursts of about 57 MB at
31.1, 62.3 and 93.6 s, and each burst pushed fsync to 30 to 42 ms. That
reproduces the 31.2 s cycle and its direction. On an idle host it does not
reproduce the soak's 59 to 409 ms. What adds the rest under soak load is
unproven. The probe and writers ran on CPU 5 in the harness role; this
measures the disk path, not the store's CPU placement. D12 checks pass for
the 317.025 s window: load 0.28 at start, CPU 5 24.38% busy, largest outside
process 0.1299 core, outside total 0.1676 core, 1,531 swap-in pages, no
throttle rise, zero placement errors and exact restoration.

**Tile a.** This is not a retried request. No tile-a request id appears twice
in the store trace. A repeated id would get the stored reply
(`crates/goliath-store/src/state.rs:258-266`). The chain, in UTC:

1. 17:44:06.871: crosser E17BotX20 finishes a d-to-b hand-over. The store
   records tile b as its owner.
2. 17:44:10.511: tile b kicks its players with `server_shutdown`.
3. 17:44:10.515: the proxy logs `lost backend "b" ... moved to "a" in 1 ms
   (servers tried: 1)`. Meridian's fallback (`meridian-proxy/src/fallback.rs`)
   is on by default (`meridian-config/src/fallback.rs:17-25`). The fixture's
   proxy config (`scripts/goliath-cluster:322-327`) does not set it.
4. Tile a logs X20 as a normal login at (1024.5, -60.0, 1024.5), not a held
   arrival. The player had no local generation there, so
   `cluster_register_players` (`cluster.rs:201-250` in the patched tile source)
   exported it and sent `CreateUuid` with a fresh request id.
5. The store refused with `AlreadyExists` (`state.rs:423`), which is correct.
   Tile a treats any reply other than `Committed(Generation(0))` as fatal
   (`cluster.rs:233-238`) and panics in its tick prelude (`dedicated.rs:1340-1346`).
   Its 96 bots go down with it.

The store rows carry no operation type, so which tile-a mutation at store
552.11 to 552.17 s is the `CreateUuid` is inferred. A hand-over of E17BotX4
from b to a also completed placement on a while b's sidecar refused release
for the expired lease. The proxy raised an ALERT. That event is retained
and not analysed further here.

**Platform.** The keeper weakness and the registration weakness exist on
both machines. This machine exposes them because its barrier tail is long:
it shares one slow volume (XFS on VDO) between the store's journal and about
2.9 MB/s of fixture logs, and its 31 s writeback bursts reach hundreds of
milliseconds. For the Mac soak to pass and this one to fail, the Mac's
store stalls stayed under the 200 ms read cap, which its 41 ms maximum
shows, and no tile lost its lease, so the fallback path never ran there.
Neither Mac document records a fallback or `AlreadyExists` event.

**Fixes, ranked.** As proposed at the time of the diagnosis.

1. Code: give the renewal client a per-phase cap of lease minus margin
   (900 ms) instead of 200 ms at `lease.rs:215`. The call deadline still
   bounds every call, and grants stay anchored to their original send, so
   authority is never extended. A renewal can then ride out a store stall
   of up to about 600 ms. The same client makes tile startup's `Directory`
   call (`lease.rs:219`), which covers M3 seed 240012 (231.8 ms reply against
   a 207.6 ms read). Test: a stub store that replies after 400 ms is
   granted before the call deadline, and one that never replies still
   expires at the keeper deadline. This changes `horizon-client`, so it
   needs a rebuild, a new bundle, a new soak and a fresh M3 campaign.
2. Code: when registration is refused, disconnect that one player and keep
   the tile running, at `cluster.rs:233-238`. Transport and authority errors
   stay fatal. Test: a fresh login of a uuid the store already holds is
   disconnected, and the tile keeps ticking with authority. Same rebuild as
   fix 1.
3. Condition: set `[fallback] on-kick = false` and `on-crash = false` in the
   fixture's proxy config. The frozen `meridian --check` accepts both keys.
   In a sharded world, a tile's players cannot be served by another tile
   without a hand-over. This contains tile a's cascade with the frozen bundle,
   but b's players are still lost. A short cell must confirm that wall
   crossings do not use this path.
4. Condition: take the bulk logs off the store's volume. Set the tiles'
   `[anticheat] log = ""` (documented as no file), and send tile and proxy
   stderr to tmpfs for collection afterwards. The journal stays on the data
   volume under `--sync full`. In isolation this removes the burst tail
   (p99.9 19.2 instead of 39.8 ms). Its effect at soak load is unmeasured.
   Moving the store's directory to the root NVMe was not tried: runs stay on
   the data volume, the root volume's idle fsync tail is no better (max 48 ms),
   and both volumes share one physical disk.

Fixes 3 and 4 change no binary; the reviewer has to decide whether they keep
the cell comparable under D9 and D12. Fixes 1 and 2 address the weaknesses
themselves. Without fix 1, any stall that holds renewal replies past 200 ms
for about 650 ms can still end a tile, as in a1. Fixes 1 and 2 became
`final-r2`. Fixes 3 and 4 were not applied to the standard fixture; fix 4's
log move became the labelled diagnostic on `final-r2`.

### Soak commands

The builder ran each once:

```sh
export GOLIATH_WINDOW_OWNER=E28S
python3 -u -B scripts/e25-soak-control --name e28-soak-r1 --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28S --collect-only
python3 -u -B scripts/e25-soak-control --name e28-soak-r1-a1 --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-retry-plan-r1-a1.json --owner E28S --collect-only
```

For a fresh reviewer attempt on `final`, not yet run, use the current
collector with the retired-thread fix and the same frozen workload. This does
not reproduce or reclassify the old collector failure. The runner acquires and
releases its own window.

```sh
export GOLIATH_WINDOW_OWNER=E28R
.local/bin/host-window build E28R -- python3 -B scripts/e28-log-integrity-tests
.local/bin/host-window build E28R -- python3 -B scripts/e28-soak-tests
python3 -u -B scripts/e25-soak-control --name repro-e28-soak-r1 --bins .local/bin/goliath-e28/final --data-dir docs/experiments/data/E28/reviewer-soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
# Only after a full collection and release:
python3 -B scripts/e25-soak-readback --name repro-e28-soak-r1 --data-dir docs/experiments/data/E28/reviewer-soak --owner E28R
```

Do not manufacture a final endpoint or run the full readback on a shortened
collection. Keep every failure and use a new name for any authorized attempt.

## M3 campaign on final

Status: paused at 72 of 1,000 seeds. The first jobs-4 crash campaign failed
on two seeds at startup; the serial refusal matrix passes. Both ran between
`e28-soak-r1` and its retry. A separately registered jobs-1 campaign, run
after the retry, reached a clean pause after 72 completed trials with no
failed trial. Its next window has not started. M3 on `final-r2` is
deferred (see [M3 on final-r2](#m3-on-final-r2)). No M3 run has a reviewer
reproduction.

The campaign preserves E24's original E13 fault kernels, seed 240001,
1,000 crossings, 12 partition cases and six stale-login cases. Jobs 4 share
one four-core server cpuset and one client core. Each low-population fixture
does not receive its own CPU budget. Whole-window D12 checks decide validity;
startup pressure or a failed trial is retained, never silently retried.
M3 uses its original direct `e4-client` path, without capacity bots or front
shims. Resume is allowed only after a clean `paused-window`, with every
admitted trial complete and the schedule unchanged.

### Jobs-4 crash campaign and refusal matrix

The [complete M3 readback](data/E28/m3/e28-m3-results-r1.json) retains all
12 admitted crash trials, including both failures, and all 18 matrix cases.
The crash campaign stops at its first observed failed trial and drains the
other admitted work. Ten trials complete and pass the original E13 item
checker, covering 30 named stacks and 310 units, with zero duplicates,
unaccounted losses or stale files. Two trials fail before their scheduled
fault. The remaining 988 seeds are unattempted, not passes.

| Seed | Scheduled fault | Outcome |
| --- | --- | --- |
| 240001 to 240010 | Original seeded phase/victim schedule | 10 complete recovery audits |
| 240011 | fsync, target | Cluster-clock initialization failed before fault |
| 240012 | install, both | Tile d exited during startup with `Resource temporarily unavailable` |
| 240013 to 241000 | Unchanged retained schedule | Not attempted |

Jobs 4 was selected to preserve E24's protocol, sharing one D12 budget rather
than receiving four budgets. It is not qualified on this machine. The precise
resource or timing cause of the startup failures is unproven. No failed seed
was rerun and the campaign was not resumed. A new, separately authorized
lower-concurrency campaign is needed to establish the 1,000-seed item.

The first window also retains one placement snapshot pairing a server
executable and server cgroup with allowed CPU 5. That raw window is invalid,
not retroactively repaired. Placement capture had read process status,
cgroup and executable at different instants. Future capture brackets those
values, retries observed migration/exec transitions at most four times, and
reports unresolved observations as unproven. A known pre-exec launcher is
charged to its declared destination only after joining that cgroup. Stable
wrong placement still fails. Tests cover both cases.

The separate serial matrix completes all 12 partition and six stale-login
cases. The original checker confirms zero accepted stale cuts, placements,
CAS operations or returning logins, with the old processes still alive and
the replacements running. Its D12 window is valid. Intentional crash/partition
lease traces retain framing metadata without claiming zero authority events
from such faulted processes.

| Window | Seconds | CPU 5 busy | Largest outside process, cores | Swap-in pages | D12 | Restored |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| e28-m3-crash-w1 | 96.257 | 57.39% | 0.14966 | 2,720 | invalid placement snapshot | yes |
| e28-m3-matrix-w1 | 233.324 | 42.22% | 0.14361 | 43 | valid | yes |

Neither window has a throttle-counter rise. The matrix's outside total is
0.20266 core. Native snapshots retain all six CPU counters, frequency ranges,
memory endpoints and placements. The invalid crash row stays invalid despite
its passing outside-CPU and thermal checks.

`e24-m3-results` was run and exited 1 with `ValueError: campaign incomplete`.
It did not pack or delete any trial. The published crash and matrix archives
retain every original JSON byte sequence, individual SHA-256 and parsed
record, with byte-identical base64 round-trip checks. There is no complete
campaign result hidden behind the passing matrix.

### Startup diagnosis

The [startup diagnosis](data/E28/m3-startup-diagnosis.json) and
[request-ID timing join](data/E28/m3-startup-read-overlap.json) identify
seed 240012's EAGAIN on the blocking Directory response read. The store
response took 231.83 ms: about 93.95 ms queued followed by a shared 137.66 ms
durable barrier, beyond the client's 207.61 ms read failure. This is a real
finding, not noise or a task-limit rejection. Seed 240011's clock writer
used tile-a authority after its renewal outage; original rejection bytes
were not retained, so expired-fence attribution is distinguished from a
direct wire capture. Frozen-store live/expired controls confirm the expected
fencing responses. Future startup records capture the exact clock replies.

Three serial and 12 concurrent startup-only diagnostics all pass. That does
not reproduce the original full fault workload or prove concurrency alone
causal. All 60 stable native placements have allowed CPUs and last processor
on 0 to 3. The old CPU-5 observation mixed reads across a wrapper migration
and exec; the checked cgroup write completes before native exec. Its original
invalid window is retained, not silently reclassified.

### Jobs-1 campaign, paused

The [fresh jobs-1 registration](data/E28/m3-fresh-jobs1-r2-plan.json) keeps
the exact seed, phase and victim schedule and the native binaries. It lowers
avoidable simultaneous store and fsync pressure without changing timeouts,
leases or fences, and adds no startup retries.

At the reviewer's instruction this window was allowed to drain at its budget
without being killed. It ended as a clean `paused-window` after 72 completed
trials, zero failures, in 1,209.790 seconds. The
[verified pause record](data/E28/m3/e28-m3-j1-r2-w1-pause.json) retains every
completed trial hash. D12 is valid, CPU 5 is 36.97% busy and policy is
restored. The original E13 checker verifies all 72 completed item audits. The
other 928 seeds are not passes. No next window starts while the soak's
cluster failures await direction.

### M3 commands

The builder ran these, serially, with no resume:

```sh
export GOLIATH_WINDOW_OWNER=E28S
python3 -u -B scripts/e24-m3 campaign --name e28-m3-crash-w1 --data-name e28-m3-crash --window-name e28-m3-crash-w1 --evidence-dir .local/evidence/E28/m3 --bins .local/bin/goliath-e28/final --cpu-mode pin --seed 240001 --crossings 1000 --jobs 4 --budget-seconds 1200
python3 -u -B scripts/e24-m3 matrix --name e28-m3-matrix --data-name e28-m3-matrix --window-name e28-m3-matrix-w1 --evidence-dir .local/evidence/E28/m3 --bins .local/bin/goliath-e28/final --cpu-mode pin --budget-seconds 1200
.local/bin/host-window build E28S -- python3 -B scripts/e24-m3-results --evidence-dir .local/evidence/E28/m3 --data-dir docs/experiments/data/E28/m3 --campaign-data e28-m3-crash --campaign e28-m3-crash-w1 --matrix-data e28-m3-matrix --matrix e28-m3-matrix --name e28-m3-results-r1 --pack
python3 -u -B scripts/e24-m3 campaign --name e28-m3-j1-r2-w1 --data-name e28-m3-j1-r2 --window-name e28-m3-j1-r2-w1 --evidence-dir .local/evidence/E28/m3 --bins .local/bin/goliath-e28/final --cpu-mode pin --seed 240001 --crossings 1000 --jobs 1 --budget-seconds 1200
```

Reviewer commands, not yet run. They reproduce the same jobs-4 configuration,
including its risk of startup failure, and are not a recommendation to replace
the failed campaign with fewer jobs.

```sh
export GOLIATH_WINDOW_OWNER=E28R
python3 -u -B scripts/e24-m3 campaign --name repro-e28-m3-crash-w1 --data-name repro-e28-m3-crash --window-name repro-e28-m3-crash-w1 --evidence-dir .local/evidence/E28/reviewer-m3 --bins .local/bin/goliath-e28/final --cpu-mode pin --seed 240001 --crossings 1000 --jobs 4 --budget-seconds 1200
python3 -u -B scripts/e24-m3 matrix --name repro-e28-m3-matrix --data-name repro-e28-m3-matrix --window-name repro-e28-m3-matrix-w1 --evidence-dir .local/evidence/E28/reviewer-m3 --bins .local/bin/goliath-e28/final --cpu-mode pin --budget-seconds 1200
# This rejects an incomplete or failed crash campaign:
.local/bin/host-window build E28R -- python3 -B scripts/e24-m3-results --evidence-dir .local/evidence/E28/reviewer-m3 --data-dir docs/experiments/data/E28/reviewer-m3 --campaign-data repro-e28-m3-crash --campaign repro-e28-m3-crash-w1 --matrix-data repro-e28-m3-matrix --matrix repro-e28-m3-matrix --name repro-e28-m3-results-r1 --pack
```

If, and only if, a window ends as a clean `paused-window`, with all admitted
trials complete, valid D12 evidence and restored policy, use a fresh `-w2`
run/window name and `--resume-from` the immediately preceding campaign.
Preserve its data name, seed, crossings, jobs and binary hashes. Do not resume
the failed builder window or any actual failed seed.

## Fixes for the soak failure

Status: built and gated by the builder. Measured since on `final-r2`: the soak
failed, the log diagnostic ran its full 30 minutes, and in the frontier check
425 passes twice while 450 fails once and loses a tile once. See
[Measurements on final-r2](#measurements-on-final-r2).

Two tile patches follow the final stack (FINAL-09,
`horizon-13-lease-renewal-r2.patch`, patch 10), in this order:

1. `horizon-22-lease-read-deadline.patch` fixes the renewal read cap. The
   lease client's per-phase cap goes from 200 ms to lease minus margin,
   900 ms at the defaults. Renewals still pass `call_before` with the call
   deadline, which is the keeper deadline minus the 50 ms RPC slack, so the
   call deadline binds first. A grant still counts from its original send
   (`LeaseKeeper::observe`), and the post-check still refuses a grant
   observed after the old deadline. A slow reply can therefore install a
   grant only inside the window that grant already allowed. The action gate
   reads and latches the keeper deadline under the state lock, and the
   keeper never holds that lock across store IO, so an outstanding read
   cannot delay the latch. The renewal loop moves unchanged into its own
   function, `keep`, so the tests can run it against a stub store. rustfmt
   reformats the moved block; no statement changed.
2. `horizon-23-registration-refusal.patch` fixes the registration panic.
   When the store answers a new login's `CreateUuid` with `AlreadyExists`,
   the tile disconnects that one player
   (`multiplayer.disconnect.duplicate_login`), marks it released and logs
   `E28_REGISTRATION_REFUSED`. A released player gets no generation, is not
   sent to the store again, and the leave path writes no player file, stats
   or advancements for it. The store checks the tile's fence before its
   player table, so this refusal says nothing about the tile's authority.
   Every other reply, stale fence and expired lease included, still returns
   an error, and the tick prelude still panics on it while the local lease
   allows actions.

A refused guest leaves through the normal leave path with the same `Guest`
value. The hub boxes it into `HostEvent::Left` (`lan/hub.rs:2323-2326`), the
game handles that event (`game/lan.rs:2194`) and calls `save_guest`
(`game/lan.rs:2222`). `save_guest` writes the player file, autosave and
native save only for `placed && !released` (`game/lan.rs:1554`), and
`save_guest_records` returns early for a released guest (`game/lan.rs:1717`).
Tile-cut checkpoints also skip released guests (`game/dedicated/tile_cut.rs:902`).
The tile's only player-store mutation is the `CreateUuid` in
`cluster_register_players`, which skips released guests, so no disconnect
path in the tile writes to the store. Hand-over writes belong to the sidecar
and were not re-examined here. Line numbers refer to the final-r2 tree.

Every user of the lease client changes. The cluster player client
(`cluster.rs`, 3 s and 3 attempts) is a separate handle and does not.

| Call | Before | After |
| --- | --- | --- |
| `RenewLease` | 200 ms read, then `WouldBlock` | Waits until the call deadline, at most keeper deadline minus 50 ms |
| Startup `Directory` | 200 ms per phase, 600 ms per call | 900 ms per phase, 2.7 s per call; M3 seed 240012's 231.8 ms reply now succeeds |
| Startup `CheckLease`, `BeginTakeover` | Same as `Directory` | Same as `Directory` |
| `ClaimLease` | A reply after 200 ms failed and the 10 s claim loop retried | A reply before sent + 900 ms installs a grant counted from that send; later ones still fail `observe` |

The tests run the real renewal loop against a loopback stub store and sample
the action gate every millisecond. A check that allowed actions is always
timed before the deadline, and the first refusal falls at or after it,
within 250 ms.

| Test | What it shows |
| --- | --- |
| `a_stalled_grant_before_the_call_deadline_counts_from_its_original_send` | A 400 ms reply is installed. Its deadline is at most the store's receive time plus 900 ms, so it counts from the send, not the reply. The old 200 ms cap would have dropped it. |
| `a_grant_arriving_after_the_deadline_is_never_installed` | A 1,200 ms reply never moves the deadline, and the gate closes on the original deadline. |
| `an_unanswered_renewal_cannot_hold_the_gate_open` | With a read outstanding until the call deadline, no gate check takes 50 ms or more, and the gate closes on time. |
| `margin_guard_charges_twice_the_lease_per_thousand` | The non-macOS margin guard rejects 999,000 ns and accepts 999,001 ns for a 500 ms lease. |
| `startup_directory_waits_past_the_old_read_cap` | A 300 ms `Directory` reply succeeds. |
| `a_refused_registration_disconnects_only_that_login` | The refused login is released, unmarked and sent a disconnect. A login beside it registers at generation 0. Neither is retried. |
| `refusals_that_are_not_about_the_player_stay_fatal` | Ten other store faults, `Committed(1)` and `Ok` all return an error and leave the login untouched. |

The test for an outstanding read covers a read that ends at the call
deadline. A read can outlast the keeper deadline only through a process
pause, which no test simulates. In that case the gate still latches on its
own clock, and the post-check refuses a grant observed late.

**Gates.** `boat-home run --wait -- scripts/boat-gates final-r2` passed on a
fork of the gate home with rustc 1.99.1 and `--profile fast`: fmt; lease
tests, 9 passed; warm tests, 3 passed; clippy `-D warnings` on the lib, no
diagnostics; registration tests, 2 passed. The runner rebuilt the stack from
the pin and checked every patch hash and the compiled sources against
[final-r2-inputs.json](data/E28/final-r2-inputs.json). Lease timing tests
ran on shared sandbox vCPUs, not this host.

**Bundle.** `.local/bin/goliath-e28/final-r2` is final's layout with a new
`horizon-client`, built here in release with final's settings (ThinLTO, one
codegen unit, six jobs, no incremental, empty `RUSTC_WRAPPER`; cargo took
34 min 14 s). A first attempt was stopped by a service restart about five
minutes in, before any binary existed; its partial target was deleted and
the build ran once more from cold. Every other
file is a hard link to final's bytes. The [build record](data/E28/build-final-r2.json)
holds the pins, the ordered patch hashes, the boat gate results and the
binary hashes. Its failed gates are final's inherited root and proxy lint
drift. The [gate disposition](data/E28/gate-disposition.json) now lists
final-r2 for those three entries and nothing else.

| Binary | SHA-256 |
| --- | --- |
| `horizon-client` (final-r2) | `673fd535a946ae0b909e5ba62e897f141fccde006eb00b637058894bf06b7162` |
| `horizon-client` (final) | `1614a5f60b87b44e1cb08c28aae6925539dc30ff5572c9fe06e723cfe3f2e9a0` |
| Others | Unchanged from final |

The final-stamps and final-lite tiles run the same keeper and registration
code, and patches 22 and 23 touch no file those variants change. Neither
variant was rebuilt. A latency or lite measurement under lease load would
need them rebuilt with the fixes.

**Runners.** The soak, frontier and M3 runners accept `final-r2` through
`goliath_bundle.validate_server_stack`. It validates the bundle against the
build manifest, accepts `final` and `final-r2` only, and requires `final-r2` to
differ from `final` in `horizon-client` alone. The runners validate again after
each cell as before. The soak plan and M3 window rows record the bundle name
and the manifest hash; frontier and soak rows carry `bundle: final-r2`. Five
new cases in `e28-soak-tests` cover the guard. All ten suites pass: 165
cases in the first run and 168 after the diagnostic option below, with four
retained Mac skips each time.

Commands:

```sh
boat-home run --wait -- scripts/boat-gates final-r2
boat-home done
E28_WINDOW_NAME=E28F python3 -B scripts/e28-build run final-r2-release .local/src/horizon-tiles -- cargo build --locked --release --no-default-features -p horizon-client --bin horizon-client
python3 -B scripts/e28-build freeze-r2 .local/evidence/E28/boat-gates/final-r2-20261109T191000Z.json
python3 -B -c 'import sys; sys.path.insert(0, "scripts"); import goliath_bundle; print(goliath_bundle.validate(".local/bin/goliath-e28/final-r2")["binaries"]["horizon-client"])'
```

`scripts/e28-build run` builds through `.local/bin/host-window build E28F`
with `CARGO_INCREMENTAL=0` and `RUSTC_WRAPPER=` empty.

## Measurements on final-r2

### Soak on final-r2

Status: `e28-soak-r2` failed at 1,216 s on a real cluster event and stays
failed. `e28-soak-r2-diag-logs` is a labelled diagnostic that tests the
cause; it is not the M4 soak row, whatever it shows. Both are builder runs
with no reviewer reproduction. Neither was rerun.

**The run.** `e28-soak-r2` used the [registered plan](data/E28/soak-r2-plan.json)
and the unchanged fixture: 425 players, pin, direct final-stamps bots, the
same logs on the same volume and proxy fallback at its defaults. Tiles a and
d stopped together at loaded second 1,216.02 with `renew-keeper-deadline`.
The [readback](data/E28/soak/e28-soak-r2-result.json) joins the lease
traces to the store's request trace by request id. Times are loaded seconds.

| Event | Time | Detail |
| --- | ---: | --- |
| Store barrier A starts | 1,215.368 | One command, tile b's renewal 5100; fsync takes 462.9 ms |
| Tile d sends renewal 5085, tile a sends 5106 | 1,215.372 | Call budget 599.9 and 599.8 ms |
| Store receives both | 1,215.373 | 5 ms into barrier A; both queue 459 ms |
| Barrier B starts with both renewals | 1,215.832 | fsync takes 181.1 ms |
| Call deadline on a and d | 1,215.972 | Keeper deadline minus the 50 ms RPC slack |
| Tiles report `overall client deadline elapsed` | 1,216.006 | 633 ms after the send |
| Store dispatches both grants | 1,216.013 | 640.1 and 640.4 ms after receipt; both committed |
| Keeper deadline on a and d | 1,216.022 | `authority-stop` 0.1 and 0.3 ms later |

The stall was 644 ms, two slow barriers back to back. A renewal can wait
600 ms: the lease is 1,000 ms, the margin 100 ms, renewals go out every
250 ms and the call deadline sits 50 ms before the keeper deadline. The
store's replies left 40 ms after the call deadline and 9 ms before the
keeper deadline. Tile b's renewal was the command inside barrier A. It went
out 0.3 ms before the barrier began, came back after 464.9 ms and installed.
Tile c sent 172 ms into the stall, shared barrier B, got its grant after
468.2 ms and installed. Only a and d, which arrived 5 ms too late for
barrier A, had to wait for both. No clock-anomaly event appears.

**What the fixes did.** The read-cap fix works as designed and was not
enough here. Before the fatal stall, 13 renewals took longer than 200 ms to
answer (207 to 475 ms) and all 13 installed. On the final bundle each of
those ended as `WouldBlock`. That includes a 406.7 ms barrier followed by an
87.8 ms one at 903 s, which all four tiles rode out. The registration fix
was not exercised: when a and d dropped their players, the proxy had no live
tile to move them to.

| Report item | `e28-soak-r2` |
| --- | --- |
| Renewals answered after more than 200 ms | 18 up to the stop. 16 installed, slowest 475.1 ms. 2 did not: the fatal pair at 633 ms. After the stop, b and c had two more (241 and 205 ms), both installed |
| Store barriers over 50 ms | 124 of 24,462 up to the stop; 32 over 100 ms, 3 over 200 ms, 2 over 400 ms; maximum 462.9 ms |
| `E28_REGISTRATION_REFUSED` lines | 0, and 0 tile panics |
| Proxy fallback events | 204 lost-backend lines within 61 ms (a 96, d 108). 191 had no other server. The proxy offered 13 crossers from d to a, which refused the connection. None moved |
| Tile authority stops | 2, tiles a and d, both `renew-keeper-deadline` |

The slow barriers again follow the 31.2 s writeback cycle from the earlier
diagnosis. Folded on 31.2 s into 12 bins, 90 of the 124 barriers over 50 ms
fall in one bin, and 37 of 38 cycles have at least one. Barrier quantiles
match the earlier attempts: p50 12.4 ms, p99 35.9 ms, p99.9 112.2 ms.

**Pass line up to the failure.** There is no minute-30 endpoint, so the
line is not met. What was observed before the stop:

| Line | Result |
| --- | --- |
| Crossings running | 376 to 382 activations in each of 20 full minutes |
| Duplicated items | 0; all 42 cargo items present once after recovery, none missing |
| Owner violations | 0 in 19 audits, 0 generation regressions, 5 crossing-during-query brackets left unresolved |
| RSS, minute 5 to the last sample at 1,215.4 s | Store +15.79% (33.08 to 38.30 MiB). Proxy -39.99%, tiles -13.45% to -53.47%, sidecars -0.24% to -1.76% |

Reported separately: owner-status TPS samples span 19.946 to 20.050 over
the four tiles. Raw tick P99 over the 23,217 ticks per tile retained before
the stop is 40.7 / 43.5 / 43.2 / 42.7 ms with zero drops; the slowest single
ticks are 464 to 531 ms. Cargo recovery ran after the failure: b and c
checkpointed, a and d refused the clean cut with an expired lease, and all
four recovered. The whole window is 1,505.889 s. D12 checks pass: CPU 5
73.52% busy, largest outside process 0.1419 core, outside total 0.2281
core, no throttle rise, exact restoration.

Two memory results need care. The store grew 15.79% because its receipt
window fills about 34 s after the minute-5 sample at this player count, as
in `e28-soak-r1`. It then stays flat. On that evidence the store would fail
the 10% line at minute 30 even without the lease loss. The proxy and tile
RSS fell by tens of megabytes at a time while the host's swap-in counter
rose by 117,828 pages. The host swaps to compressed memory and the sampler
does not record per-process swap, so RSS in this run does not bound memory
and the falls prove nothing about the processes.
The [D12 addendum](../decisions.md#d12-addendum-the-soaks-memory-line-on-this-platform),
recorded after these runs, judges the line on resident plus swap from the
later of minute 5 and a full receipt window. The figures here are not that
ruled figure.

**Diagnostic: bulk logs off the store's volume.** `e28-soak-r2-diag-logs`
repeats the soak with one change, [registered](data/E28/soak-r2-diag-logs-plan.json)
before the run. Each tile's stdout/stderr log (which holds the input trace)
and its `logs/anticheat.log` go to a tmpfs directory through symlinks at
their usual paths. The store's journal and snapshots, the worlds, the tile
cuts, the proxy and sidecar state and logs, and the lease and store traces
stay on the data volume. The option is `--bulk-logs-tmpfs` on the soak
runners. It is off by default, accepted only for names containing `-diag-`,
and touches no binary. It refuses to start below 7 GiB of free tmpfs or
9 GiB of available memory, fails if a tile replaces a link, and removes the
directory at the end. Three cases in `e28-log-integrity-tests` cover it. The
archive walk and the failure-evidence reader open these logs by their usual
paths and follow the links; no audit reads them any other way. A 60-second
labelled smoke at 425 players confirmed eight intact links and a
byte-complete archive before the long run. The smoke is not a measurement.

The diagnostic ran its full 1,800 s with no authority stop. The
[comparison](data/E28/soak/e28-soak-r2-diag-logs-comparison.json) uses the
same store and lease traces for both runs.

| | `e28-soak-r2` | `e28-soak-r2-diag-logs` |
| --- | ---: | ---: |
| Loaded seconds | 1,216.0, ended by the stop | 1,800.0 |
| Store barriers | 24,462 | 38,854 |
| Barrier p50 / p99 / p99.9 ms | 12.43 / 35.86 / 112.20 | 11.94 / 31.16 / 47.49 |
| Barriers over 50 ms | 124 | 31 |
| Barriers over 100 ms | 32 | 0 |
| Barriers over 200 ms | 3 | 0 |
| Barriers over 400 ms | 2 | 0 |
| Largest barrier ms | 462.9 | 88.8 |
| Longest run of consecutive slow barriers ms | 644.0 | 120.3 |
| Renewal replies | 19,417 | 28,755 |
| Renewal replies over 200 ms | 18 (16 installed, 2 not) | 0 |
| Slowest renewal reply ms | 633.3 | 117.3 |
| Renewal reply p99 ms | 62.2 | 40.1 |
| Share of over-50 ms barriers in the fullest of 12 cycle bins | 72.6% (90 of 124) | 19.4% (6 of 31) |
| Cycles of 31.2 s with a barrier over 50 ms | 37 of 38 | 27 of 57 |
| Tile authority stops | 2 | 0 |

With the bulk logs off the volume the 31-second cycle is no longer visible.
The 31 remaining barriers over 50 ms spread over ten of the twelve bins, and
barriers over 30 ms are flat at 6% to 11% per bin. No barrier reached 100 ms
and no renewal took 200 ms. Cut at the first 1,216 s for an equal span, the
diagnostic has 22 barriers over 50 ms against 124. This supports the
earlier diagnosis: writeback of about 2.75 MiB/s of fixture logs on the
store's volume produces the stalls. It is one run. It does not show what
else can stall this volume, and it leaves the 600 ms limit where it is.

The diagnostic's pass line, for the record and not as the M4 row:

| Line | Result |
| --- | --- |
| Crossings running | 375 to 381 activations in each of the 30 minutes |
| Duplicated items | 0; all 42 cargo items present once after the clean checkpoint and recovery |
| Owner violations | 0 in 29 audits, 0 generation regressions |
| RSS, minute 5 to minute 30 | Fails on the store: +14.25% (33.30 to 38.05 MiB). Proxy -40.97%, tiles -15.71% to -49.91%, sidecars -13.33% to -15.09% |

The [readback](data/E28/soak/e28-soak-r2-diag-logs-e25-result.json) verdict
is `rss-lines-failed`. Game-time TPS is 20.000 / 19.976 / 20.000 / 20.000.
Long-window raw tick P99 is 44.5 / 45.1 / 43.2 / 43.4 ms with zero drops.
There are no authority or clock-anomaly events and all lease traces have
complete framing. The store's receipt window filled at loaded second 334.8,
30.4 s after the minute-5 sample, which accounts for its growth. The same
RSS caution applies, more strongly: the logs took 5,395 MB of tmpfs by the
end, available memory fell from 11.87 GB to 3.64 GB, and the swap-in counter
rose by 497,887 pages. After the directory was removed, 9.14 GB was
available. The whole window is 2,129.304 s. D12 checks pass: CPU 5 80.56%
busy, largest outside process 0.1552 core, outside total 0.3241 core, no
throttle rise, exact restoration.

The first readback call went through `e25-analysis-exec`, whose CPU budget
refuses to start without a D12 window. Its child readback still ran to the
end and published the result. A second, direct call collided with it and
changed nothing. Both logs are kept.

**What is unproven.** The soak line on this machine is not met on the
standard fixture. The store's memory line fails in both runs for a fixture
reason that predates the fixes. The registration fix has no live-fire
evidence. RSS falls are unexplained beyond the host swapping. The frontier
row that lost a tile has no store trace.

The builder ran each command once:

```sh
export GOLIATH_WINDOW_OWNER=E28R2
python3 -u -B scripts/e25-soak-control --name e28-soak-r2 --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-r2-plan.json --owner E28R2 --collect-only
python3 -u -B scripts/e25-soak-control --name e28-soak-r2-diag-logs --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-r2-diag-logs-plan.json --owner E28R2 --collect-only --bulk-logs-tmpfs
python3 -B scripts/e25-soak-readback --name e28-soak-r2-diag-logs --owner E28R2 --data-dir docs/experiments/data/E28/soak
```

The failed run's readback and the comparison come from two private scripts,
`.local/evidence/E28/finalize-soak-r2.py` and
`.local/evidence/E28/compare-soak-r2-diag.py`. Both read only retained
evidence.

Reviewer commands, not run by the builder. The runner takes and releases its
own window. Run the readback only after a full collection.

```sh
export GOLIATH_WINDOW_OWNER=E28R
.local/bin/host-window build E28R -- python3 -B scripts/e28-soak-tests
.local/bin/host-window build E28R -- python3 -B scripts/e28-log-integrity-tests
python3 -u -B scripts/e25-soak-control --name repro-e28-soak-r2 --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/reviewer-soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-r2-plan.json --owner E28R --collect-only
python3 -B scripts/e25-soak-readback --name repro-e28-soak-r2 --data-dir docs/experiments/data/E28/reviewer-soak --owner E28R
python3 -u -B scripts/e25-soak-control --name repro-e28-soak-r2-diag-logs --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/reviewer-soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E28/soak-r2-diag-logs-plan.json --owner E28R --collect-only --bulk-logs-tmpfs
python3 -B scripts/e25-soak-readback --name repro-e28-soak-r2-diag-logs --data-dir docs/experiments/data/E28/reviewer-soak --owner E28R
```

A standard-fixture reproduction may survive 30 minutes or stop at another
cycle: one stall in 38 cycles was fatal here and one more came within
110 ms of the limit. Either outcome is a result. For the diagnostic, check
that about 9 GiB of memory is available first and that
`/dev/shm/goliath-e28-logs-<run>` is gone afterwards.

### Frontier check on final-r2

Status: builder result, no reviewer reproduction. The tile binary changed in
`final-r2`, so four v3 direct-bot cells under pin check that capacity did not
move. Servers come from `final-r2`, bots from `final-stamps` with tracing
off, and the pass rule is the one above. Each row records `bundle: final-r2`.

| Cell | Players | Valid | Pass | TPS | Raw P99 ms | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| [frontier-r2-425-r0](data/E28/frontier/e28-frontier-r2-425-r0.json.gz) | 425 | yes | yes | 19.998 / 19.998 / 19.998 / 19.998 | 40.647 / 42.110 / 38.407 / 39.264 | pass |
| [frontier-r2-425-r1](data/E28/frontier/e28-frontier-r2-425-r1.json.gz) | 425 | yes | yes | 20.002 / 20.000 / 19.999 / 20.012 | 36.697 / 38.414 / 37.461 / 37.683 | pass |
| [frontier-r2-450-r0](data/E28/frontier/e28-frontier-r2-450-r0.json.gz) | 450 | no | no | - | - | tile a lost its lease 32.7 s into the steady window; no measurement |
| [frontier-r2-450-r1](data/E28/frontier/e28-frontier-r2-450-r1.json.gz) | 450 | yes | no | 19.996 / 19.996 / 20.010 / 20.003 | 53.040 / 54.334 / 53.208 / 53.380 | raw P99 >= 50 ms; no drops |

| Cell | Server cores | Proxy cores | Proxy threads | Bot cores | CPU 4 busy | CPU 5 busy | Outside total steady | Swap-in pages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| frontier-r2-425-r0 | 2.718 | 0.721 | 871..877 | 0.295 | 33.41% | 55.88% | 0.288 | 24 |
| frontier-r2-425-r1 | 2.643 | 0.702 | 872..879 | 0.278 | 31.93% | 50.08% | 0.209 | 92 |
| frontier-r2-450-r0 | - | - | - | - | 34.62% | 52.88% | - | 46 |
| frontier-r2-450-r1 | 3.058 | 0.812 | 922..928 | 0.332 | 37.38% | 54.41% | 0.350 | 744 |

Against the final-bundle v3 rows, 425 is unchanged within the spread of two
repeats: raw P99 36.7 to 42.1 ms here against 35.2 to 40.5 ms, and server
cores 2.64 and 2.72 against 2.62 and 2.67. 450 was mixed on final (one pass
at 42.1 to 46.2 ms, one fail at 49.8 to 53.0 ms). Here it has one fail at
53.0 to 54.3 ms and one invalid cell, so 450 still does not qualify and
nothing suggests the fixes cost capacity. Two cells per step cannot resolve a
difference of a few percent. The stable step stays 425.

`frontier-r2-450-r0` is a cluster failure, not a harness fault, and was not
rerun. Tile a refused its clean shutdown cut with `local cluster lease
expired`. The proxy logged 107 lost-backend lines for a within 0.1 s and one
ALERT for a hand-over from a to b that a's sidecar refused for the expired
lease. Bot groups a and cross exited 1. D12 checks pass (CPU 5 52.88% busy,
46 swap-in pages). The frontier harness keeps no lease or store trace, so the
store stall behind this expiry is not observed in this row. The soak above
shows the mechanism on the same fixture. The
[failure evidence](data/E28/frontier/e28-frontier-r2-450-r0-450-failure-native.json.gz)
is retained.

The ledger records `frontier-r2-450-r0` as not valid, because every cell whose
status is not `complete` is not valid. The frontier rule says players must
stay online, so a tile lost during a valid D12 window is a failure of the
step, not a missing measurement (audit finding 4). Either way 450 has no pass
on `final-r2`.

The rows' harness hashes differ from the final-bundle v3 rows in two files.
`goliath_bundle.py` gained the final-r2 guard. `goliath_platform.py` carries
the retired-thread counter fix made after the v3 staircase. The kernel,
layout, bots and pass rule are the same. The
[ledger](data/E28/frontier/e28-frontier-r2-staircase.json) comes from
`e20-results`.

The builder ran, once per cell and in this order (425 r0, 425 r1, 450 r0,
450 r1):

```sh
export GOLIATH_WINDOW_OWNER=E28R2
.local/bin/host-window quiet E28R2 e28-frontier-r2-425-r0
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/frontier --cpu-mode pin --direct-bots --counts 425 --name e28-frontier-r2-425-r0
.local/bin/host-window release E28R2
python3 -B scripts/e20-results e28-frontier-r2 --baseline 200 --data-dir docs/experiments/data/E28/frontier
```

Reviewer commands, not yet run:

```sh
export GOLIATH_WINDOW_OWNER=E28R
for e28_count in 425 450; do
  for e28_repeat in 0 1; do
    e28_label="repro-e28-frontier-r2-${e28_count}-r${e28_repeat}"
    .local/bin/host-window quiet E28R "$e28_label"
    python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final-r2 --data-dir docs/experiments/data/E28/reviewer-frontier --cpu-mode pin --direct-bots --counts "$e28_count" --name "$e28_label"
    .local/bin/host-window release E28R
  done
done
python3 -B scripts/e20-results repro-e28-frontier-r2 --baseline 200 --data-dir docs/experiments/data/E28/reviewer-frontier
```

A cell that loses a tile exits 1 and stays in the ledger as invalid. Keep it
and do not repeat it.

### M3 on final-r2

<!-- The measuring builder replaces this subsection with its results when the M3 run on final-r2 is done. -->

Status: deferred by the reviewer, not started. No `final-r2` M3 trial, pilot
or matrix case has run on this machine, and no `e28-m3-r2-` data exists. The
campaign on `final` stays paused at 72 of 1,000 seeds. The M3 runner accepts
`final-r2` and its window rows record the bundle name and manifest hash; that
change is covered by tests only. Whether two clusters at a time are sound on
this machine with the 900 ms read cap is untested.

## Harness defects found by audit

An independent audit recomputed the baseline, frontier, latency and lite rows
from the raw files and read the harness code. The headline numbers hold. It
found these defects in the harness that produced the rows above. They are
known limitations of this report. None changes the baseline, the frontier or
the ratio. Fixes are in progress; at the time of writing none is in
`scripts/` with a test, so every row here was measured with these defects
present.

| # | Defect | Effect on published rows |
| ---: | --- | --- |
| 1 | The CPU 5 saturation rule averages over join and warmup, not only the measured minute. | `latency-pin-via-r0` had CPU 5 at 97.66% over the measured minute and still read valid; it is a tick failure in a harness-saturated cell and never counted. The v1 rows read valid with CPU 5 at 100% in the measured minute; v1 is excluded from the frontier. v3 rows are at 46 to 54%. No published verdict changes. |
| 2 | "Server cores" sums registered processes. Under pinning the four CPUs carried about 0.3 core more. | Interpretation only. The failure explanation is corrected under [Why the cluster fails](#why-the-cluster-fails). |
| 3 | The bundle guard ignores pending gates when accepted lint failures are present. | `final-stamps` was frozen with five pending gates and validated. Those gates passed later (exit 0 in [its build record](data/E28/build-final-stamps.json)). No row changes. |
| 4 | A cell where the cluster dropped players is recorded as not valid instead of as a failure. | `frontier-r2-450-r0` on `final-r2`. Under the frontier rule it is a failure; 450 has no pass on `final-r2` either way. The final-stack frontier is unaffected. |
| 5 | The placement check accepts any group whose CPUs match its own leaf, not the group a role belongs to, and only checks at the end mark. | No misplaced role in any capacity row. The lite clock probe exited before the end mark and has no placement record; CPU 0 was 94% busy and CPUs 1 to 4 idle in both calibrations, so it ran on a server core. |
| 6 | Restore gaps: SIGQUIT, SIGUSR1, SIGUSR2 and SIGALRM skip cleanup; a vanished driver cgroup blocks removal; other top-level cgroups and all kernel threads keep CPUs 0 to 4 unchecked. | No row known to be affected. The soak diagnosis names the volume's kernel threads bursting on the fixture's cores, and their CPU is not reported separately. |
| 7 | Three Mac paths changed without a platform switch. | No Mac row was rerun. See [What changed on the Mac paths](#what-changed-on-the-mac-paths). |
| 8 | On Linux the outside rule does not count the measurement driver. | Plain baseline only; the driver used 0.03 core in every baseline cell. No validity result changes. |

The audit's smaller items include an unhandled ESRCH in the process sampler
(it fails closed, the same class as the first soak's crash), no positive
control for the Linux limiter, and the soak runners' hard-coded 425 players.
They change no row.

## What changed on the Mac paths

The plan said Mac behaviour stays unchanged. These three changes are not
behind a platform switch:

- `scripts/e20-frontier` always starts capacity shims with `--forward-only`.
  A Mac rerun of an E20 frontier cell would use a different shim from the
  published E20 rows. The shim sits outside both caps, so server numbers
  should not move, but the harness is no longer the one that produced 650.
- `scripts/e20-summary` uses `thread_role`, which adds a `player supervisors`
  role and matches `terrain-manager` by prefix. A Mac rerun of the derived
  summary reclassifies those threads.
- `scripts/e18_d9_wire.py` applies the 8,192-byte handshake buffer cap only
  while the handshake is incomplete, and a zero-byte send now raises.

Everything else the audit compared is equivalent on Darwin by reading. None
of it was run on a Mac, and no Mac result in E17 to E27 was remeasured.
