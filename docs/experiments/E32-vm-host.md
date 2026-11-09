# E32. A second test bed on a VM host

Status: builder results, pending reviewer reproduction. The harness mode and the
rebuild work. The measurements do not give a baseline, a frontier or a ratio,
because the VM host's disk stalls on durable writes for seconds at a time. Every
cell that started is kept, and most are failed or invalid.

## Question

Measurements queue on one workstation. Can an idle virtual machine carry them
as a second test bed? And does the 30-minute soak, which fails on the
workstation when its deduplicating volume stalls the store's fsync, pass on a
different volume?

vCPU numbers are a third platform. They are compared only with this host's own
baseline and never with the Mac's or the workstation's.

## Results

1. The harness has a VM mode and the stack rebuilds on the VM host from the
   upstream pins in 26 minutes. Every compiled source tree is byte-identical to
   the workstation's E28 record, and the standard store binary came out
   byte-identical too.
2. The soak does not pass here. With the standard store, all four tiles lost
   their leases within 9 seconds of starting, before any player joined. The
   store's barrier reached 719 ms in its first 8.6 seconds. The lease and fsync
   coupling is therefore not special to the workstation's volume.
3. With the E29 store, which answers granted renewals from memory, no tile lost
   its lease in either run, through barriers of up to 2,955 ms. The run still
   failed after about three minutes of load: hand-overs wait for the journal,
   the tiles' cut queues filled behind a train of one-second barriers, and 9 of
   the 42 crossing players dropped.
4. No baseline, frontier or ratio. On the standard fixture, disk stalls make
   bots late and add ticks of hundreds of milliseconds at any load. Of seven
   plain cells on disk one is valid. Of four cluster cells none is. The plan's
   stop rule applied: bots went late before the servers failed, here from the
   disk and not from the bot core.
5. This host is not faster for this workload. Plain-09 costs 0.163 core at 50
   players, against 0.167 on the workstation, and it overloads between 250 and
   300 players.

## Machine

| Item | Value |
| --- | --- |
| CPU | 8 vCPUs of an AMD Ryzen 9 7950X, KVM guest, one thread per vCPU |
| Memory | 31 GiB, zram swap |
| Disk | one ext4 volume on a virtio disk, 273 GB free |
| OS | NixOS 26.11, Linux 6.18, cgroup v2 with the cpuset controller |
| Toolchain | rustc 1.99.1 (58cf8f9c3), the workstation's compiler |
| Missing | no cpufreq governor, no thermal-throttle counters |

The clock source is `kvm-clock`. `systemd-detect-virt` reports `kvm`.

## The VM mode

`GOLIATH_HOST_PLATFORM=vm` selects it. Nothing infers it from the machine. The
default is the workstation, whose checks are unchanged.

| CPUs | Runs |
| --- | --- |
| 0 to 3 | tiles, proxy, store, sidecars, or the one plain Horizon process |
| 4 | bots |
| 5 to N-1, here 5 to 7 | harness and every other process on the host |

The workstation still requires exactly CPUs 0 to 5. The harness-CPU rule now
applies to the mean busy fraction of the harness set and every CPU is reported.
On the workstation that set is CPU 5 alone, so its rule and its wording are the
same as before.

A virtual machine has no governor to set and no throttle counters to read. The
window skips the governor step and records `governor: unavailable (virtual
machine)`. In place of the throttle rule, a cell is judged on CPU steal, the
time the hypervisor ran something else while a vCPU wanted to run. Each sample
reads per-CPU steal from `/proc/stat`. A cell is invalid if steal on any
fixture CPU, 0 to 4, is over 1% of that CPU's time. Steal on the harness CPUs
is reported and does not void a cell. Rows carry `platform: vm`.

A physical host is never valid through this path. Each snapshot records what
`systemd-detect-virt` said. A window refuses to open in VM mode on a host it
calls physical, a row whose snapshots say `none` is invalid, and the VM
manifest refuses to validate there.

D9's other rules are unchanged: load 4 or less at the start, and no outside
process over half a core.

### Bundle identity

Binaries built on the VM host have their own hashes, so the VM host has its own
trust anchor, [the E32 build manifest](data/E32/build-manifest.json), and its
own bundle folder, `.local/bin/goliath-e32/`. The same strict checks run
against it. A bundle validates only against the manifest of the host it was
built on:

- the bundle must sit in the selected host's folder,
- the manifest must name the selected host,
- the bundle's own record must name it too, so a forged manifest entry for
  copied bytes still fails.

The VM host builds only the bots of the stamped stack. They are a bundle of
their own, `stamped-bots`, with [their own disabled-trace
proof](data/E32/direct-bots-proof.json). The proof runs E28's guard checks
against the VM host's frozen sources, which equal the workstation's. The
workstation's proof and the two files it hashes are untouched.

### Tests

`scripts/e32-platform-tests` has 28 tests: both layouts, the steal rule and its
1% boundary, missing steal coverage, the physical-host refusals, the harness
mean rule, the unchanged workstation path, cross-host bundle refusals, the
store-variant guard and the planning guard.

Twenty existing suites give the same result before and after the change, run on
the VM host. Three of their tests need the workstation's frozen bundles and
error there both times. The workstation's five bundles were validated in place
with the changed module, read-only.

A live check, `e28-platform-check --name harness-vm-live`, passed in a host
window: four workers allowed on CPUs 0 to 3 and one on CPU 4, slices narrowed to
5 to 7 and restored, and the SIGTERM path restored too.

## Build records

`scripts/e32-build` reuses the pins, patch stacks and commands of
`scripts/e28-build`. Before each build it compares the compiled inputs of the
tree with the workstation's E28 record and stops on any difference.

| Tree | Files | Against | Result |
| --- | ---: | --- | --- |
| Horizon, plain-09 stack | 2,986 | `build-plain-09.json` | identical |
| Meridian, reactor-1 stack | 269 | `build-final.json` | identical |
| Horizon tiles, final plus stamp overlays | 2,998 | `build-final-stamps.json` | identical |
| Horizon tiles, final | 2,996 | `build-final.json` | identical |
| Horizon tiles, final-r2 | 2,996 | `build-final-r2.json` | identical |

Settings are E28's with eight jobs: release, ThinLTO, one codegen unit,
`CARGO_INCREMENTAL=0`, empty `RUSTC_WRAPPER`, one target dir per workspace, one
build at a time. The store uses its own recipe: release, two jobs, no profile
overrides.

| Step | Seconds |
| --- | ---: |
| plain-09 server and bots | 587.3 |
| store | 1.8 |
| proxy, sidecar, e4 client | 116.0 |
| stamped bots | 74.9 |
| final-r2 tile | 472.9 |
| fmt | 8.8 |
| lease tests, fast profile, 9 passed | 263.6 |
| warm tests, 3 passed | 0.2 |
| registration tests, 2 passed | 0.2 |
| whole run, clone to freeze | 1,560.5 |

Only those four cheap tile gates ran here. Clippy and the root, proxy, plain
and bot gates did not. They ran for E28 on the workstation and on boat. Each
[build record](data/E32/build-final-r2.json) says which gates did not run.

| Bundle | Binary | SHA-256 |
| --- | --- | --- |
| plain-09 | `horizon-client` | `919f14add061fcd4b915ca1b9ed43ad2d1d99b937f7bba93428192a81e631c24` |
| plain-09, final-r2 | `horizon-bots` | `71821b1b5aaf8ae25fc82a907eae93969202765e29f72122297d8c9d9722aece` |
| stamped-bots | `horizon-bots` | `19b80307e0625649e62d2b7df1a101a3fa8171d8d2bdd35d31ce72d2994b26f4` |
| final-r2 | `horizon-client` | `810480d20937b627afdaff2b7c8422df23df9d32640c55736cc77a14535cdeea` |
| final-r2 | `goliath-store` | `ac3c8075a6ec92e52a0201925a2b896c4ffb2803526473630bced68cd35ba888` |
| final-r2 | `meridian` | `dcaf397e9c8de0919d3e54adbfd81c81d2f349148f27edcf37caa8249efdf3d1` |
| final-r2 | `meridian-sidecar` | `4d280dc818d78ea868bbabeb66c3175cc1f13dfd0dcc74123927d9b7883d1b82` |
| final-r2 | `e4-client` | `4b94811a9f254beed89c9cffb740fbdf02896b0700058bac4fe26f253cec42e8` |
| final-r2-e29 | `goliath-store` | `6879c72fbb8806a343f621e9c44968c08082da4a098142b4983d7b80b7b7dd00` |

The standard store's hash equals the workstation's final-r2 store. The
workstation froze its store before one atomic call in the store binary was
renamed from `fetch_update` to `try_update`. The two compile to the same bytes.
So `e28-soak-r2` and `e32-soak-r1` ran the same store executable on two disks.
The Horizon and Meridian binaries differ from the workstation's, as expected.

`final-r2-e29` is final-r2 with only `goliath-store` replaced. The store was
built from branch `e29-lease-renewal` at commit `76fbd58`, exported with `git
archive` into its own directory. That worktree had uncommitted edits in three
store files at the time. They are not in the build. The server-stack guard
names, per derived stack, the only executables in which it may differ from its
base. Here that is `goliath-store` alone. The tile was not rebuilt. E29 changes
store-side code in two libraries the tile also compiles and leaves the client
and codec files byte-identical. One test on the E29 branch failed once in nine
workspace runs and was being traced, so every result with this store is
provisional.

The manifest's SHA-256 is
`66e29200ea12d4d1ed0fedc1147a20d08aacdde1cfcc87bd5d7bbd011cf5ace6`. Before
`final-r2-e29` was added it was `840301c5...`, which the first cells record.
The bot proof was regenerated once, when its check against the manifest hash
was dropped so the manifest could grow. The first two cluster cells and `e32-soak-r1`
record the earlier proof, `909caf2e...`, which is in this branch's history.

## The disk

The first plain smoke ran at 20 TPS with a raw P99 of 9.1 ms and was still
invalid. Bots reported 16 late rounds with a worst lateness of 978 ms, and four
server ticks took 126, 136, 468 and 873 ms. Bots used 2.6% of CPU 4. Three
probes without any fixture found the cause.

| Probe | Result |
| --- | --- |
| CPU-only spinner on CPUs 2, 4 and 6, idle and beside a disk load | no gap over 5 ms |
| 4 KiB append and fsync, idle host, 300 samples | p50 6.0 ms, p99 22.9 ms, max 188 ms |
| the same beside four buffered writers, 60 s, 857 samples | p50 6.1 ms, p99 256 ms, max 12,311 ms, 16 over 200 ms |
| buffered appends on ext4 beside that fsync loop | max 2,298 ms, three over 200 ms in each of four writers |
| buffered appends on tmpfs in the same minute | max 0.3 ms |

The vCPUs do not pause. The disk does. One fsync took 12.3 seconds, and plain
buffered writes on the same volume stalled with it for up to 2.3 seconds. The
tiles and the bots write logs every tick, so a stall on this volume stops them
without any fsync of their own. The workstation's probe on its deduplicating
volume reached 42 ms beside comparable writers. What sits behind the virtual
disk is not visible from the guest. The cause on the host side is unproven.

Two diagnostics ran with the whole run folder on tmpfs. That turns the store's
journal fsync and the tile cuts' flushes into no-ops, so they remove the
durability cost entirely and confirm the cause only. Both were clean: plain 50
valid with no late round and a longest tick of 13 ms, cluster 100 complete with
raw P99 near 8 ms. They are not measurement rows. Two ran instead of one
because the reviewer's limit of one arrived after both had finished.

## Soak

All three runs use the unchanged soak: 425 players, pinning, direct stamped
bots with tracing off, run folders, logs and the store's journal on the ext4
volume, store `--sync full`. Each plan was registered before its run.

| Run | Store | Ended | Cause | Authority stops |
| --- | --- | --- | --- | --- |
| [e32-soak-r1](data/E32/soak/e32-soak-r1-barriers.json) | standard | 8.6 s after the store started | all four leases expired at startup | 4 |
| [e32-soak-r2-e29](data/E32/soak/e32-soak-r2-e29-barriers.json) | E29 | 63 s, during registration | the harness's planning guard | 0 |
| [e32-soak-r2-e29-a1](data/E32/soak/e32-soak-r2-e29-a1-barriers.json) | E29 | 181 s of bot runtime | 9 of 42 crossing players dropped | 0 |

None is a 30-minute result.

### Store barriers and renewal replies

The workstation column is `e28-soak-r2` up to its first authority stop at
1,216 s, from that run's published result, which is not yet in this branch.

| | Workstation e28-soak-r2 | e32-soak-r1 | e32-soak-r2-e29 | e32-soak-r2-e29-a1 |
| --- | ---: | ---: | ---: | ---: |
| Store | standard | standard, same bytes | E29 | E29 |
| Seconds of barrier trace | 1,216 | 8.6 | 61.9 | 196.7 |
| Barriers | 24,462 | 137 | 563 | 1,399 |
| p50 ms | 12.4 | 7.0 | 11.6 | 5.4 |
| p99 ms | 35.9 | 392.5 | 348.4 | 365.3 |
| p99.9 ms | 112.2 | 719.0 | 896.4 | 1,704.5 |
| Maximum ms | 462.9 | 719.0 | 896.4 | 2,954.6 |
| Over 50 ms | 124 | 10 | 49 | 53 |
| Over 100 ms | 32 | 6 | 30 | 34 |
| Over 200 ms | 3 | 6 | 17 | 24 |
| Over 400 ms | 2 | 1 | 4 | 13 |
| Renewal replies | 20,833 | 95 | 962 | 3,061 |
| Replies over 200 ms | 20 | 10 | 0 | 0 |
| Slowest reply ms | 633.3 | 618.8 | 169.5 | 51.4 |
| Renewals answered from memory | none | none | 962 of 962 | 3,061 of 3,061 |

Renewal reply times come from the tiles' lease traces and cover every tile
process in the run. With so few barriers, p99.9 in the first two VM columns is
the maximum.

The typical barrier here is faster than the workstation's and the tail is far
worse. The workstation saw three barriers over 200 ms in 20 minutes. This disk
produced six in under nine seconds.

### e32-soak-r1

Tiles d, c and b stopped with `renew-keeper-deadline` at 5.3, 6.5 and 7.6
seconds into their lease traces, and tile a refused actions at 8.6 seconds. The
cargo setup was then refused and the runner stopped. Ten of 95 renewal replies
took over 200 ms. The read-cap fix in final-r2 lets a renewal wait up to its
call deadline. Six barriers over 200 ms in under nine seconds used that up. This is a
cluster failure, so there was no retry.

The cluster smoke before it failed the same way at startup with `cluster clock
initialization failed`. The frontier runner keeps no store trace, so the stall
in that cell is inferred from this one.

### e32-soak-r2-e29

Startup survived barriers of 700 and 896 ms, and no tile stopped in 62 seconds
that held 17 barriers over 200 ms. The slowest renewal reply was 169.5 ms on
tile b, while the store answered every renewal within 0.23 ms of receiving it.
Where the other 169 ms went is unproven. The run was stopped by the harness, not
the cluster: its planning guard saw 73 accepted mutations in one rolling second
against a planning rate of 72 a second.

### The planning guard

The guard protects the store's receipt window. The store keeps the last 9,216
mutation receipts so that a retried command gets its original answer. The
longest client retry budget is 27 seconds, and E24 requires the cap to hold
four times the mutations of one budget:

```text
1 + 4 * ceil(27 * rate) <= 9216
mutations allowed in any 27 s at 4x = floor(9215 / 4)        = 2303
largest rate the cap covers         = floor(9215 / (4 * 27)) = 85 a second
```

The 72 a second in the harness was E24's measured workload rate plus an
allowance. It was checked over every rolling second as a conservative stand-in
for the 27-second budget.

| Trace | Most in one second | Most in 27 seconds | Share of 2,303 |
| --- | ---: | ---: | ---: |
| e32-soak-r2-e29, startup and registration | 80 | 541 | 23% |
| e32-soak-r2-e29-a1, whole trace | 31 | 558 | 24% |
| Workstation e28-soak-r2, loaded window | 32 | 610 | 26% |
| Workstation e28-soak-r2, whole trace | 47 | 990 | 43% |

The guard stopped the first run at 73, before the second reached its full 80.
Registration commits as fast as the barrier lets it, and commits bunch behind
barrier stalls. A one-second burst of 80 does not threaten a window sized for
27 seconds: the worst 27 seconds held 541 mutations, under a quarter of the
4x budget and 6% of the cap. The workstation's whole-trace figures include its
journaled renewals, about 16 a second, which the E29 store no longer journals.

The retry therefore checks the budget itself: at most 27 times 85, or 2,295,
accepted mutations in any rolling 27 seconds. The cap, the retry budget and the
4x margin are unchanged. A soak can use this only if its registered plan
carries the rate, the window and the cap, and the cap covers the rate. The
default check is untouched. [The plan](data/E32/soak-plan-r2-e29-a1.json) holds
the arithmetic. The loaded-window readback still applies the 72 a second rule.
It never ran, because no collection completed.

### e32-soak-r2-e29-a1

The run joined all 425 players and ran its loaded window. At 61 seconds the
audit showed 425 acting identities and no owner violation. No tile lost its
lease, and the slowest of 3,061 renewal replies took 51.4 ms.

From store second 162 the barriers ran in a train: 2,955 ms, then 1,296, 828,
896, 815, 813, 1,263, 1,704, 1,103, 405, 1,391 and 833 ms over 26 seconds.
Hand-over commits wait for the barrier in the E29 design. The proxy logged 115
`cut queue is full` answers from tiles and 36 wall refusals, and disconnected 7
players. After 181 seconds of bot runtime the crossing group had 33 of 42
players online. The four interior groups kept all 383 players, with no late
round. The runner stopped on the dropped players. The guard stayed inside its
bound, with 558 of 2,295.

The whole window is also invalid by the VM rule, for one reason: steal on CPU 4
was 1.02%. The run folder's policy was restored exactly.

So the E29 store removes the lease failure on this disk and exposes the next
one. A lease now survives a barrier of almost three seconds. A hand-over does
not survive a run of one-second barriers. Whether that second failure would
appear on the workstation, whose barrier tail is much shorter, is unproven.

## Baseline and frontier

There is no bracket and no ratio. The pilots show why.

Plain-09, standard fixture, all on disk:

| Cell | Players | Valid | Pass | TPS | Raw P99 ms | Longest tick ms | Server cores | Main thread | Late rounds | CPU 4 busy | Max fixture steal | Reason |
| --- | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| smoke-plain-50-r0 | 50 | - | - | - | - | - | - | - | - | - | - | preflight: manifest missing, no fixture started |
| [smoke-plain-50-r0-a1](data/E32/baseline/e32-smoke-plain-50-r0-a1.json.gz) | 50 | no | no | 19.997 | 9.08 | 873 | 0.163 | 0.132 | 16 | 2.6% | 0.03% | bots late |
| [pilot-plain-150-r0](data/E32/baseline/e32-pilot-plain-150-r0.json.gz) | 150 | no | no | 19.997 | 29.23 | 224 | 0.559 | 0.468 | 8 | 8.8% | 0.25% | bots late |
| [pilot-plain-200-r0](data/E32/baseline/e32-pilot-plain-200-r0.json.gz) | 200 | no | no | 19.997 | 45.73 | 499 | 0.820 | 0.701 | 5 | 11.8% | 0.33% | bots late |
| [pilot-plain-250-r0](data/E32/baseline/e32-pilot-plain-250-r0.json.gz) | 250 | yes | no | 19.997 | 52.83 | 61 | 1.010 | 0.894 | 0 | 12.5% | 0.07% | raw P99 at least 50 ms |
| [pilot-plain-300-r0](data/E32/baseline/e32-pilot-plain-300-r0.json.gz) | 300 | no | no | 15.101 | 75.09 | 2,721 | 1.073 | 0.947 | 5 | 11.4% | 0.05% | overload, bots late |
| [pilot-plain-400-r0](data/E32/baseline/e32-pilot-plain-400-r0.json.gz) | 400 | no | no | 7.268 | 205.67 | 354 | 1.128 | 0.989 | 3 | 10.5% | 0.05% | overload, bots late |
| [pilot-plain-500-r0](data/E32/baseline/e32-pilot-plain-500-r0.json.gz) | 500 | no | no | 3.588 | 348.75 | 403 | 1.148 | 0.992 | 6 | 13.9% | 0.05% | overload, bots late |

The first smoke never started a fixture. A local sync helper had deleted the
manifest it had just fetched from the VM host. The helper was fixed and the
records were restored byte-identical. Its log is the only record of that
attempt.

Cluster, direct bots, standard fixture, all on disk:

| Cell | Players | Servers | Outcome | Raw P99 ms a / b / c / d | Server cores | Proxy cores | Bot cores | CPU 4 busy | Harness mean busy | Max fixture steal |
| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| [smoke-final-r2-100-r0](data/E32/frontier/e32-smoke-final-r2-100-r0.json) | 100 | final-r2 | failed at startup: cluster clock initialization | - | - | - | - | - | 1.7% | 0.17% |
| [pilot-e29-300-r0](data/E32/frontier/e32-pilot-e29-300-r0.json.gz) | 300 | final-r2-e29 | complete, invalid: CPU 4 steal 1.18% | 46.9 / 45.6 / 40.8 / 54.5 | 2.082 | 0.756 | 0.224 | 22.3% | 6.0% | 1.18% |
| [pilot-e29-400-r0](data/E32/frontier/e32-pilot-e29-400-r0.json) | 400 | final-r2-e29 | failed while joining: all four tiles panicked | - | - | - | - | - | 1.9% | 0.36% |
| [pilot-e29-500-r0](data/E32/frontier/e32-pilot-e29-500-r0.json.gz) | 500 | final-r2-e29 | complete, invalid: CPU 4 steal 1.41% | 57.9 / 54.7 / 57.9 / 53.9 | 3.286 | 1.158 | 0.370 | 37.3% | 8.8% | 1.41% |

In the 400-player pilot every tile panicked in its tick prelude with `cluster
authority failed: Resource temporarily unavailable`. That is a read timeout on a
store call during player registration. The frontier runner keeps no store
trace, so the barrier behind it is inferred, not observed. It is not a lease
loss. The E29 store answers renewals early and nothing else.

Both complete cluster pilots held 20 TPS on every tile with no dropped tick and
one late bot round. Their raw P99 barely moves between 300 and 500 players,
41 to 55 ms against 54 to 58 ms, while server CPU rises from 2.08 to 3.29
cores. A P99 that does not follow load is set by the disk, not by the tiles.
Both are also invalid on steal.

Tmpfs diagnostics, not measurement rows:

| Cell | Players | Result |
| --- | ---: | --- |
| [diag-tmpfs-plain-50-r0](data/E32/diag/e32-diag-tmpfs-plain-50-r0.json.gz) | 50 | valid, pass, 19.998 TPS, raw P99 8.68 ms, longest tick 13 ms, no late round |
| [diag-tmpfs-final-r2-100-r0](data/E32/diag/e32-diag-tmpfs-final-r2-100-r0.json.gz) | 100 | complete, pass, raw P99 8.1 / 8.1 / 8.5 / 7.8 ms, 0.621 server cores |

What the pilots do say about this host:

- Plain-09 passes TPS at 200 with a raw P99 of 45.7 ms and misses P99 at 250
  with 52.8 ms. Its baseline would land near the workstation's 200. Two valid
  passes and two valid failures are missing, so this is a region and not a
  bracket.
- The proxy costs more here. At 300 players it used 0.756 core, where the
  workstation's proxy used 0.65 at 400.
- The bot core is not the limit. CPU 4 was at most 37.3% busy.
- Steal on the fixture CPUs rises with the fixture's own load: 0.03% at 50
  plain players, 0.9 to 1.4% under a cluster. Whether another guest or this
  guest's own IO causes it cannot be seen from inside.

## What is unproven

- A baseline, a frontier and a ratio for this host. None exists.
- A 30-minute soak on either store. The longest run reached about three minutes
  of load.
- The cause of the disk stalls on the host side, and whether they are permanent.
- The barrier behind the 100-player startup failure and the 400-player panic.
  Those runners keep no store trace.
- Whether hand-overs fail under barrier stalls on the workstation with the E29
  store.
- Whether steal over 1% under cluster load is outside interference or this
  guest's own IO. The 1% line voids such cells either way.
- The E29 store itself: one of its tests failed once in nine runs, and its
  crash campaign has not been rerun. M3 was not run here.
- The three existing tests that need the workstation's frozen bundles did not
  run on the VM host. The workstation's bundles were validated in place
  instead. No cell ran on the workstation with the changed harness.
- The VM mode was exercised on one guest with eight vCPUs.

## Reproduction

On the VM host, in the synced tree. The reviewer uses its own `repro-*` names.

```sh
export GOLIATH_HOST_PLATFORM=vm GOLIATH_WINDOW_OWNER=E32 E28_WINDOW_NAME=E32B
export CARGO_INCREMENTAL=0 RUSTC_WRAPPER=
ulimit -n 524288   # the workstation's shell default; this host's is 1024

# Build, gate and freeze. Refuses to overwrite a published manifest.
python3 -u -B scripts/e32-build all
python3 -B scripts/e32-direct-bots-proof --proof docs/experiments/data/E32/direct-bots-proof.json

# The E29 store: export the commit into its own directory first.
git -C <e29 worktree> archive 76fbd58 crates Cargo.toml Cargo.lock clippy.toml | tar -x -C .local/src/goliath-e29
python3 -u -B scripts/e32-build freeze-e29

python3 -B scripts/e32-platform-tests
.local/bin/host-window quiet E32 harness-vm-live
python3 -B scripts/e28-platform-check --name harness-vm-live
.local/bin/host-window release E32

# One plain cell and one cluster cell, each in its own window.
.local/bin/host-window quiet E32 e32-pilot-plain-250-r0
python3 -B scripts/e17-d9-baseline --bins .local/bin/goliath-e32/plain-09 --data-dir docs/experiments/data/E32/baseline --cpu-mode pin --cell 250 --repeat 0 --out docs/experiments/data/E32/baseline/e32-pilot-plain-250-r0.json
.local/bin/host-window release E32
.local/bin/host-window quiet E32 e32-pilot-e29-300-r0
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e32/final-r2-e29 --data-dir docs/experiments/data/E32/frontier --cpu-mode pin --direct-bots --counts 300 --name e32-pilot-e29-300-r0
.local/bin/host-window release E32

# The soaks take their own window.
python3 -u -B scripts/e25-soak-control --name e32-soak-r1 --bins .local/bin/goliath-e32/final-r2 --data-dir docs/experiments/data/E32/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E32/soak-plan-r1.json --owner E32 --collect-only
python3 -u -B scripts/e25-soak-control --name e32-soak-r2-e29 --bins .local/bin/goliath-e32/final-r2-e29 --data-dir docs/experiments/data/E32/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E32/soak-plan-r2-e29.json --owner E32 --collect-only
python3 -u -B scripts/e25-soak-control --name e32-soak-r2-e29-a1 --bins .local/bin/goliath-e32/final-r2-e29 --data-dir docs/experiments/data/E32/soak --cpu-mode pin --direct-bots --players 425 --campaign-plan docs/experiments/data/E32/soak-plan-r2-e29-a1.json --owner E32 --collect-only --planning-mutation-rate 85 --planning-window-seconds 27

# Readback, evidence and ledger.
python3 -B scripts/e32-soak-readback --name e32-soak-r1 --evidence .local/evidence/E32/soak/e32-soak-r1 --out docs/experiments/data/E32/soak/e32-soak-r1-barriers.json
python3 -B scripts/e32-publish --name e32-soak-r1 --evidence .local/evidence/E32/soak --data-dir docs/experiments/data/E32/soak
python3 -B scripts/e32-results
```

Every cell stays in the foreground. If a run is killed, run `python3 -B
scripts/goliath-cpu restore` before anything else.

The tree reaches the VM host with `rsync -a` from the file list of `git
ls-files -co --exclude-standard`, with the remote `.local` protected. The game
resources are copied separately to `$HOME/.local/share/goliath-resources/26.3`
and never enter Git.

## Data

[The ledger](data/E32/measurement-ledger.json) lists every retained cell with
its verdict, key figures, VM validity block and both hashes. Raw rows over 1 MB
are `.json.gz` with a sidecar hash of the decompressed bytes. Each soak has an
evidence index, such as [this
one](data/E32/soak/e32-soak-r2-e29-a1-evidence-index.json), with its store
trace, renewals trace, lease traces, whole-window record and native failure
evidence. Full run folders stay on the VM host.
