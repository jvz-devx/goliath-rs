# E20. Cluster frontier and proxy cost

The original reactor-1 frontier is 650 players, exactly 2.0 times the matched
325-player baseline. Both 650 rows pass and both 700 rows fail raw tick P99.
The 675 step is an unstable band, with one pass and one failure retained.
The reviewer reproduced the result. Combined-1's two 650-player pilots are
mixed, so no replacement frontier is claimed. Its full M2 regression suite
passes the separately versioned v2 analysis, with three unresolved timing
windows retained. No further Meridian build was made after profiling; the
reviewer accepted deferring the small transport experiment and handed the
checkout to E22. No high-load added-latency or whole-M4 acceptance is claimed.
The matched D9 plain baseline is 325. E20 first measures frozen reactor-1 with
all four tiles, the proxy, store and four sidecars inside four aggregate cores.
Five bot drivers share a separate one-core limiter. All runs use native release
artifacts, 30 seconds of warm-up and 60 seconds of measurement.

## Pass rule and quiet windows

Each tile must reach at least 19.9 game-time TPS and raw tick P99 strictly below
50 ms. All requested players must stay online, without failed or late bot rounds.
Raw tick buffers must have no drops. All tiles retain 64 pigs. No limiter pause
is subtracted from the raw ticks or latency. Bot STOP events are reported but
do not independently fail a cell without actual lateness, drops or saturation.
This differs from E18's conservative any-bot-STOP diagnostic flag and follows
the E20 brief's explicit pass rule.

The frontier is the highest step where both repeats pass. A higher step must
fail both repeats to close the bracket. A mixed step is an unstable band;
both rows remain in the report and it gets no additional repeats. This is the
same two-repeat rule applied to the matched baseline, which had no mixed step.
The reviewer clarified this interpretation after E20's mixed 675-player step.

Each cell acquires `.local/quiet-request` as E20, waits for E21's acknowledgement
and start load at most four, and releases the request after fixture cleanup.
No outside process may average over half a core during the cell. Published
whole-host process rows redact outside names to `outside`, retaining PID and CPU.
E21's windows are acknowledged only while E20 has no build or measurement active.

## Frozen reactor-1 staircase

The layout scales `scripts/e18_d9_layout.py::plan` without changing geometry.
Ninety percent patrol four interior squares. The other ten percent, rounded
down, cross walls and the corner. The unchanged fixture has 256 total pigs.
The test-only hostname shims and measurement controller are outside production
server CPU and are reported separately. They are not proxy implementations.

| Cell | Pass | Tile cores a/b/c/d | Proxy cores | Store cores | Server cores | Tile TPS minimum | Raw P99 maximum ms | Server/bot STOP cycles |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 500 r1 | yes | 0.3963/0.3904/0.3856/0.3958 | 0.7234 | 0.00616 | 2.3013 | 20.0023 | 24.3844 | 6/0 |
| 600 r1 | yes | 0.5257/0.4916/0.5146/0.5369 | 0.9623 | 0.00665 | 3.0415 | 19.9893 | 26.9308 | 85/0 |
| 650 r1 | yes | 0.5611/0.5533/0.5807/0.5767 | 1.0738 | 0.00764 | 3.3578 | 19.9951 | 43.7746 | 381/0 |
| 650 r2 | yes | 0.5611/0.5536/0.5510/0.5665 | 1.0210 | 0.00748 | 3.2650 | 20.0003 | 34.0396 | 103/0 |
| 675 r1 | yes | 0.6143/0.5959/0.5894/0.5792 | 1.1077 | 0.00765 | 3.4980 | 19.9949 | 44.9028 | 415/0 |
| 675 r2 | no | 0.6083/0.5940/0.6010/0.6037 | 1.1336 | 0.00782 | 3.5523 | 19.9986 | 54.1583 | 812/1 |
| 700 r1 | no | 0.6481/0.6433/0.6479/0.6504 | 1.3836 | 0.00914 | 3.9878 | 19.9166 | 74.9195 | 3205/11 |
| 700 r2 | no | 0.6368/0.6421/0.6438/0.6494 | 1.2476 | 0.00948 | 3.8339 | 19.9904 | 62.5849 | 1732/4 |

The 500 cell retains all players with no bot lateness. Four sidecars together
use 0.00333 core. All tiles have 50 readers, 64 pigs and 373 chunks. Start load
is 1.67285; maximum outside-process average is 0.05872 core during measurement
and 0.08932 over the whole cell. Server STOP wall time is 0.04129 seconds.
This was the exploratory starting step. Raw and derived results are under
`docs/experiments/data/E20/reactor1-500-r1*.json`.

The 600 cell passes, with all players online and no bot lateness. The first
700 cell fails every tile's raw P99 and retains all players, with 31.801 seconds
of server STOP wall time. That established the initial 600-to-700 exploratory
bracket. E21 received a build-only slot afterward. All subsequent baseline
cells retained the original frozen engine and proxy bytes.

The first 650-player cell passes too. Tile P99 is 38.24 to 43.77 ms. The four
sidecars use 0.00465 core combined. Each tile retains 65 readers and 64 pigs;
the chunk counts are 368/367/367/367. The server limiter stops 381 times for
3.274 seconds and the bot limiter never stops. E21 used its agreed tile-build
diagnostic and stale-warm-cut window afterward. The baseline then resumed at
675 with reactor-1 and the original engine.

The 675-player step is mixed. The first run passes every tile's P99, 39.20 to
44.90 ms. The second fails every tile's P99, 50.63 to 54.16 ms. Both keep all
675 players online without lateness and satisfy the outside-noise rule. The
second run's one bot STOP cycle does not itself explain or waive the tile-tail
failures. Server STOP wall time is 3.694 seconds in r1 and 7.422 in r2.
No additional 675 run was made. Both rows remain in the unstable band. No
numerical limit, layout or result changed.

The 650 repeat passes, with P99 27.06 to 34.04 ms and no bot STOP cycles.
Its server limiter stops 103 times for 0.891 seconds. This gives two valid
passing rows at 650. The second 700 row also fails, with P99 58.44 to 62.58 ms
and 16.193 seconds of server STOP wall time. It closes the higher failing
bracket at 700. The tested grid is 25 players; the final bracket spans 50
because 675 is unstable. All eight cells satisfy the host-noise rule and keep
every requested player online with no bot lateness. No raw tick samples drop.
The only failed capacity gate in the 675 r2 and two 700 rows is raw tick P99.
`data/E20/reactor1-campaign.json` and `data/E20/reactor1-staircase.json` retain
the full closed ledger. The reviewer received the frontier immediately.

## Independent reproduction

The reviewer repeated two 650-player rows and one 700-player row on the exact
original reactor-1 bundle, layout, caps and durations. All three pass the
host-noise rule and retain all players without bot lateness. The 650 rows pass
and 700 fails the raw tick-tail gate again.

| Reviewer row | Pass | Server cores | Proxy cores | Minimum tile TPS | Maximum raw tick P99 ms | Server/bot STOP cycles |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 650 r1 | yes | 3.2645 | 1.0187 | 19.9975 | 45.3964 | 231/0 |
| 650 r2 | yes | 3.2668 | 1.0497 | 19.9977 | 38.9593 | 248/0 |
| 700 r1 | no | 3.7793 | 1.2233 | 19.9942 | 60.7551 | 1310/1 |

The raw and derived rows are `data/E20/reviewer-reactor1-*.json`. This
reproduces the 650-player, 2.0-times frontier and the higher failing bracket.
The original 675 mixed band remains in full; it was not rerun.

## Shared scheduler and release verification

The shared-scheduler edit keeps terrain reconciliation state and errors together
in the proxy terrain owner. Existing lazy readers, hysteresis, reset handling,
refresh period and pre-CAS admission backoff are retained. Per-session timers
join their existing IO driver's timer turn. Bounded caller-owned workers perform
blocking control and backend login, never the IO driver. One session can have
at most one reconciliation in flight. Worker-queue saturation retries on the
next turn without discarding reader state. Cancellation drops the reader owner.

The normal blocking transport retains its old per-player manager path. No
authority, leases or store behavior changed. Source is not a candidate until
release gates and the E18 M2 campaign have run. No IO-cost cut has been made yet.

`patches/meridian-07-terrain-scheduler.patch` applies after patch 06 and reproduces
every source byte. Its SHA-256 is
`2160c2f13c5bbae59a570f90d559a5165dc526d97c5855c7a2448cc3474b71bd`.
Release formatting and strict Clippy passed. Release tests passed 202 config,
33 ghost, 96 handoff, 107 net, 564 proxy, two signal and 99 sidecar cases. The
same one internet test is ignored and the exact known listener-burst test is
skipped, as in E18. Five added tests cover periodic admission/cancellation,
bounded task shutdown, non-overlapping reconciliation and retry after queue
saturation. Gate commands and compressed logs are under
`data/E20/scheduler-gates-1*`.

The immutable release snapshot is `.local/bin/meridian-e20/scheduler-1`, with
proxy SHA-256
`dd47ae76a9d07046de6fb6dd04a1e0d526a3381e69851a725e28cdc32a94848f`.
The engine, bots, store, sidecar and packet-client hashes are identical to
reactor-1. Its `candidate.json` explicitly says M2 is pending and candidate is
false. No scheduler load number or M2 result is claimed yet. The E14 executable
snapshot validator passes without running a client or server.

A follow-up source guard closes the race between observing reconciliation
completion and claiming an idle worker slot. The worker's terminal state is
rechecked after the atomic claim. A completed reconciliation must not run again.
One additional test covers that terminal behavior. The follow-up is
`patches/meridian-07-scheduler-stop-race.patch` with SHA-256
`04d595a8c1620ed189ba90f51db1acd50b3036eb99f101ea6523af063f59a83a`.
It applies after the first scheduler patch; complete source byte equality
passes. Formatting, strict release Clippy, all release tests and the binary
build pass again in `data/E20/scheduler-gates-2*`. The proxy suite now has 565
passing cases; all other counts and the two known exclusions are unchanged.
The immutable scheduler-1 snapshot is unchanged and has not been used for an
M2 or capacity claim.

The current frozen scheduler is `.local/bin/meridian-e20/scheduler-2`, proxy
SHA-256 `8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad`.
Its standalone creation manifest remains immutable and says M2-pending.
The reviewer selected one full combined M2 campaign instead of a separate
scheduler-only suite. All prerequisite binary hashes in scheduler-2 remain
identical to reactor-1. The original-binary staircase
resumed at 675 after this second build, with no further builds during its
refinement or repeated endpoint cells.

## Combined validation selected by the reviewer

The reviewer selected one full campaign on the combined scheduler-2 proxy and
E21's warmtick-2 tiles, replacing the earlier separate-suite plan. The physics
failure then led to the fixed-count isolation described below.

The verified immutable bundle is `.local/bin/meridian-e20/combined-1`. Its
proxy SHA-256 is
`8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad`.
Its engine SHA-256 is
`ea4537868c90771e0d030ccfff1dbe36dace215a2c70d652dd86b38b84434090`,
copied from `.local/bin/horizon-e21-fc2868b2b/warmtick-2`. Both source component
snapshots remain unchanged. Store, bots, sidecar and packet-client hashes match
both components and the original reactor-1 prerequisites. The lease marker and
launcher hashes match too. The executable snapshot validator passes.

E21's selected engine patch is `patches/horizon-10-warm-tick.patch`, SHA-256
`2ea13f3e6941a5b56306bde91943e42e9b520814378f9f692a45427ec48d4ce4`.
Its source proof is `data/E21/stack-proof.json`, SHA-256
`de438131a8d7be828cb36eb6cf18ac1de13f84bda7f50a8269d33f1d67bcb15e`.
The tile build manifest identifies that exact proof and its completed release
gates. Combining performs hash checks and copies retained binaries; it runs no
compiler, client or server. The combined build record is
`data/E20/combined-1-build.json` and retains both input-manifest hashes.

The host order was E21's two short reader controls, the reviewer's original
650/700 reproduction with both builders idle, then the combined campaign.

After reproduction completed, the reviewer confirmed E21 had finished and
waived further acknowledgements for E20's remaining windows. This is an
explicit `--no-other-agents` switch, not a default relaxation. Every window
still creates and releases its quiet request, starts at load at most four and
retains outside-process noise sampling. No timing pause is subtracted.
The original pre-waiver frontier runner is retained byte-for-byte in
`data/E20/frontier-runner-before-ack-waiver.py`, SHA-256
`b43b9243f244b96a52c89010e8037ae4a86d926a8b926177b79c436e8904d611`.
That hash matches the executed runner hash in all original and reviewer rows.
It is a provenance copy, not a standalone entry point in that directory.

The original combined M2 campaign completed under v1. Its ledger is
`data/E20/combined1-M2.json`. The 1,000-crossing proof completed with zero
disconnects, zero correction and all 1,000 fenced CAS generations visible.
Input-gap P50/P95/P99 is 101.765/141.553/145.615 ms. All four tiles restarted;
inventory matched, only the current owner retained a player file and its
generation was 1,000. Cluster-clock packets had no backward step. The quiet
window lasted 581.551 seconds, began at load 3.764 and had a maximum outside
average of 0.0583 core. The proof's raw result is `data/E20/combined1-proof.json.gz`.

Terrain completed 100 crossings; the item, border, live-state, teleport and
lifecycle/fast-approach checks completed too. Both real-physics suites collected
56 crossings. The unchanged checker reports one resolved failure, 109 resolved
windows out of 112 and 28 of 28 sprint-packet checks. The failed particle-off
walk a-to-b window at index 0 has speed ratio 0.940197 against the unchanged
0.95 threshold. Its measured speed is 4.261561 blocks/s against a 4.532624
blocks/s no-wall reference. It has no correction, stall, missing frame or
missing server tick. Three particle-off sprint windows remain unresolved.
All native collections passed the quiet/noise checks.

The v1 campaign remains failed. Its raw runs and failed verification remain
under `data/E20/physics/combined1-*`. V1 was not changed or overwritten. The
subsequent separately versioned analysis corrects the no-wall reference, not
the trial or the 0.95 pass line.

## Fixed-count physics isolation

The reviewer requested a fixed comparison to distinguish a component regression
from one-second tick-phase noise. Before the first comparison run, E20 fixed
three full particle-off suites for each of these bundles:

- Reference, `.local/bin/meridian-e18/reactor-1`, reactor-1 plus FINAL-09.
- Proxy-only change, `.local/bin/meridian-e20/scheduler-2`, scheduler-2 plus FINAL-09.
- Tile-only change, `.local/bin/horizon-e21-fc2868b2b/warmtick-2`, reactor-1 plus warmtick-2.

Each suite covers walk, sprint, sprint-jump and fly, with 14 crossings per mode.
The order rotates reference/proxy/tiles, proxy/tiles/reference, then
tiles/reference/proxy. All nine attempts remain in the results, with no early
stop or replacement for a low ratio. The existing E14 analysis and resolution
predicates supply the resolved ratio lists, minima, medians and unresolved
counts per mode. The checker and 0.95 line remain unchanged. The reviewer,
not this comparison adapter, decides whether the metric is at its noise floor.

The precommitted plan, frozen input identities and all completed run records
are in `data/E20/physics-isolation1.json`. All nine collections completed,
with 504 resolved windows, zero unresolved windows, no corrections and no
ratio below 0.95. The summary retains every trial and complete per-mode ratio
lists. A single-leg proxy isolation had
already begun before this fixed plan was requested. That raw run is retained
as `physics/isolate-proxy-walk-off-1.json.gz` and is not counted in the balanced
three-by-three comparison.

The reviewer then fixed three full combined-1 off suites in advance. All three
completed under v1, with 168 resolved windows, zero unresolved windows, no
correction failures and no ratio below 0.95. The walk minimum/median is
0.982092/0.999999. The original failed combined run stays separate. The plan
and full results are `data/E20/physics-combined-replication1*.json`.

## Original first-window investigation

`scripts/e20-first-window` reconstructs the original index-0 walk against the
three accepted-reference index-0 walks. The actual 4.261561 blocks/s is inside
their 4.251867-to-4.308026 range. The original no-wall denominator, 4.532624,
is higher than their 4.316954-to-4.317263 baselines. Client per-physics-tick
forward advances match the reference sequences, about 0.215859 block, with
every retained tick index advancing by one and no client stall or correction.
The original whole leg takes 2870.26 ms versus its baseline's 2864.93 ms.

The original no-wall interpolation window spans about 21 step-equivalents,
while the trial spans 19.742341. Cut 16.875 ms, CAS 3.776 ms and activation
123.388 ms after client crossing are within the reference cost/timing range.
All four runs have 11 chunk and five forget packets in the crossing window,
with no unknown-underfoot sample and the same peak meshing count, 168.
Pending chunks peak at 19 versus 7/10/15 in the references. No CPU limiter is
used in these physics fixtures. Reader/prewarm connect and retire timestamps
were not retained, so no exact churn claim is made. Per-tick server positions,
velocity and client frames are in `data/E20/first-window-analysis.json`.

The reviewer identified this as a no-wall reference quantisation defect,
not an observed loss of the player's per-tick movement. The original v1
failure remains in the evidence.

## Versioned checker v2 and all-retained audit

`scripts/e20-physics-v2` leaves the trial's exact v1 one-second metric intact.
It maps the trial endpoint physics-step offsets and observed placement onto
the no-wall trajectory. It divides that baseline displacement by its own
matched elapsed timestamps. There is no fixed-one-second or nominal-20-Hz
normalization and no search for a passing ratio. A missing corresponding
observed step is unresolved. Plateau placement is the same fraction within
that tick's observed support; a single observed sample is its unique point.

The exact original case is pinned by raw SHA-256 in a unit test. Its matched
baseline spans 19.742341 steps over 0.990960128 seconds, giving 4.300436 blocks/s
and ratio 0.990960128. Trial speed stays 4.261561. Other tests prove that a
missing endpoint remains unresolved, a genuine twofold tick slowdown still
fails at ratio 0.5, and both v1 source files remain byte-identical. All four
tests pass.

V2 was applied to all 35 retained raw physics runs, including every build-10
run, reactor-1, all nine isolation suites, the original combined pair, the
three combined replications and earlier E14 pilots/matrices. No derived file
was replaced. The audit is
`data/E20/analysis-v2/physics-v2-audit-1/manifest.json`.

- No previously passing run becomes failing under v2.
- No run loses a resolved window. Build-10 remains 56/56 in each run;
  reactor-1 remains 55/56 off and 56/56 on. Every isolation and replication
  remains 56/56. The original combined pair remains 53/56 off and 56/56 on.
- An already-failing historical run gains a resolved sprint-jump failure at
  index 31, ratio 0.982415 in v1 and 0.949597 in v2. It is counted, not hidden.
- Full per-run counts, failing indices, and every resolved per-mode ratio,
  minimum and median for v1 and v2 are in [the physics report](E20-physics-analysis.md).

`scripts/e20-build10-check-v2` retains the original binary, resource, route,
generation, correction and sprint checks, and adds explicit all-run coverage
and passing-run regression guards. The original full combined off/on suite
passes v2: 112 actual crossings, 109 resolved windows, 28 sprint checks,
zero failures. The same three unresolved timing windows remain. The result is
`data/E20/analysis-v2/combined1-full-verification-v2.json`. V1's failed output
and source hashes remain unchanged.

## Combined-1 pilots and profile

The reviewer authorized two 650-player combined pilots, then stopped further
bracketing because the pair did not establish a clear capacity improvement.

| 650 players | Result | Tile cores a/b/c/d | Proxy cores | Store cores | Server cores | Tile-a P99 ms | Maximum P99 ms | Server/bot STOP cycles |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| combined r1 | fail | 0.5423/0.5312/0.5190/0.5338 | 1.0366 | 0.00715 | 3.1743 | 51.0471 | 51.0471 | 306/3 |
| combined r2 | pass | 0.5168/0.5194/0.5171/0.5337 | 0.8939 | 0.00748 | 2.9925 | 27.6346 | 39.5042 | 127/0 |

Both retain all 650 players, 65 readers and 64 pigs per tile, about 20 TPS,
zero bot lateness and zero dropped tick samples. Both satisfy the noise rule.
No 625-player bracket or replacement frontier was claimed. The official result
stays reactor-1's 650.

The read-only profile at 650 records 7.872 CPU seconds of Running sample
weight over an 8.551-second range. IO drivers account for 88.73 percent of
target CPU weight; shared control workers 6.90 percent and wall workers
4.15 percent. IO-role reads are 28.56 percent, writes 22.92 percent,
allocation/copies 13.09 percent and selector calls 5.24 percent. IO lock/wakeup
weight is only 0.33 percent. Blocked stack occupancy is not used as CPU share.

Native tile-a slow-tick spans identify LAN/guest work in the captured tails.
The largest captured line is 46.3 ms wall and 17.8 ms CPU, overlapping direct
sampling. Outside direct tile sampling, examples are 32.5/14.5 ms and
32.4/16.2 ms. The old unperturbed duration-only records cannot be assigned to
exact sampled stacks. The profile did not reproduce the 51-ms P99 failure;
no off-CPU or limiter time is subtracted from a capacity result. Full limits,
per-role frames and native spans are in [the profile proposal](E20-profile-proposal.md).

The reviewer accepted deferring a transport experiment. Its defensible expected
saving is about 0.04 to 0.08 proxy core at 650, below observed pilot variation.
No IO delta or further Meridian build was made. A later experiment needs
evidence for at least a 0.10-core win before another frontier campaign. E22's
observer/shared-view work is the next candidate, outside E20's final result.

### Upstream waypoint item

`Waypoints::update_or_create` is tile-a's leading nonwaiting sampled leaf.
Its per-receiver source connection lookup is linear. This is a separate
upstream performance lead, not proof that it caused the old slow tick.
Optimizing it would also improve plain Horizon, so D9 would require a new
matched plain baseline rather than reusing 325. No upstream submission was made.

## Publication audit, handoff and cleanup

`data/E20/final-audit-1.json` passes: 139 JSON inputs, 44,461 outside-name
entries and 28,771 outside phase rows checked. Every outside name is `outside`;
PIDs and numeric CPU evidence remain. Frozen binaries and patches match all
four retained manifests, all 35 retained physics inputs match their audit
hashes, and v1's analysis, checker and original combined input are unchanged.
Screenshots, native samples and environment traces remain under `.local/evidence/E20/`.

Seven measurement-contract tests and four v2 tests pass. Release formatting,
strict Clippy and scoped Rust release tests passed before the source handoff.
No root application crate changed, so no unrelated root Rust gate was added.

Plain `scripts/clean-builds`, without `--all`, completed and freed 0 MiB.
Root, Horizon and Meridian targets remain under their caps at 38, 2,018 and
565 MiB. Total project size was 7,844 MiB. Temporary E20 worlds and logs were
removed by each fixture. No E20 build, profiler, game client, fixture or load
remains. E20's acknowledgements were released after E22's final compatibility
window. Meridian ownership passed to E22 at the byte-verified scheduler-2 base;
E20 made no production edit after handoff.

No excluded research or GPL source was read. Nothing was committed, pushed or
sent upstream. Unrelated worktree changes were preserved.

## Frozen paths, hashes and exact reproduction

Run commands from the repository root with fresh result/output names and the
reviewer's current quiet-window allocation. `--no-other-agents` is only the
explicit E21 waiver; `--e22-peer` still waits for E22. The runner creates and
releases its own quiet request. Do not pre-create that request manually.

| Frozen artifact | SHA-256 |
| --- | --- |
| `.local/bin/meridian-e18/reactor-1/meridian` | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| `.local/bin/meridian-e18/reactor-1/horizon-client` | `b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d` |
| `.local/bin/meridian-e20/scheduler-2/meridian` and `combined-1/meridian` | `8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad` |
| `.local/bin/meridian-e20/combined-1/horizon-client` | `ea4537868c90771e0d030ccfff1dbe36dace215a2c70d652dd86b38b84434090` |
| common `meridian-sidecar` | `c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b` |
| common `horizon-bots` | `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d` |
| common `goliath-store` | `6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af` |
| common `e4-client` | `e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025` |
| `combined-1/candidate.json` | `fe6db2d21f9d0043258d460951750c333396e83d8c18c63b6d826bdf83888edf` |
| v2 all-retained manifest | `49dd5da025c3e46ef8ea0f731cd2a1f7a83cacf56693dac3cd754e999f7dc6fe` |
| full combined v2 verification | `4fd452a0c9e986b876173d99433063071d89f7d2a5f2ba2827fb2d90b27fbb78` |
| final publication/frozen audit | `f1f5470b9a013ecc0806cd043a3e0f503be069c5fc1b4eb158fa8cf01d4aa592` |

The two Meridian deltas, in order after patch 06, are
`meridian-07-terrain-scheduler.patch`, SHA-256
`2160c2f13c5bbae59a570f90d559a5165dc526d97c5855c7a2448cc3474b71bd`,
then `meridian-07-scheduler-stop-race.patch`, SHA-256
`04d595a8c1620ed189ba90f51db1acd50b3036eb99f101ea6523af063f59a83a`.
The source pin is `902dcf195e0a8bbd3f39a5f560684af00e64bf5d`.
The complete ordered stack and source-file hashes are in the frozen manifests.
The pre-ack-waiver runner is retained at hash `b43b9243f244b96a52c89010e8037ae4a86d926a8b926177b79c436e8904d611`;
the combined-pilot runner copy, `data/E20/frontier-runner-waiver-v2.py`, is
`279c0307e50498f5151d79c9955f5a83d93d34bf48fae39e7d7b69cb6c893140`.

```sh
python3 -B scripts/e20-tools-test
python3 -B scripts/e20-physics-v2-tests
python3 -B scripts/e20-frontier --name repro-reactor1-650-r1 --counts 650 --bins .local/bin/meridian-e18/reactor-1 --no-other-agents --e22-peer
python3 -B scripts/e20-frontier --name repro-reactor1-650-r2 --counts 650 --bins .local/bin/meridian-e18/reactor-1 --no-other-agents --e22-peer
python3 -B scripts/e20-frontier --name repro-reactor1-700-r1 --counts 700 --bins .local/bin/meridian-e18/reactor-1 --no-other-agents --e22-peer
python3 -B scripts/e20-frontier --name repro-reactor1-700-r2 --counts 700 --bins .local/bin/meridian-e18/reactor-1 --no-other-agents --e22-peer
python3 -B scripts/e20-results repro-reactor1
python3 -B scripts/e20-physics-v2 --name physics-v2-repro
python3 -B scripts/e20-build10-check-v2 docs/experiments/data/E20/physics/combined1-physics-off.json.gz docs/experiments/data/E20/physics/combined1-physics-on.json.gz --snapshot .local/bin/meridian-e20/combined-1 --coverage-report docs/experiments/data/E20/analysis-v2/physics-v2-repro/manifest.json --output docs/experiments/data/E20/analysis-v2/combined1-full-repro-v2.json
python3 -B scripts/e20-deep-profile --name combined1-profile650-repro --bins .local/bin/meridian-e20/combined-1 --no-other-agents --e22-peer
python3 -B scripts/e20-profile-analysis --name combined1-profile650-repro
python3 -B scripts/e20-final-audit --name final-audit-repro
scripts/clean-builds
```

## Stored raw data

Raw result files larger than 1 MB are stored gzip-compressed as `*.json.gz`. Compression is lossless, and every SHA-256 recorded in this report refers to the decompressed bytes. To rerun a summary script on one of them, decompress it first with `gzip -dk <file>.json.gz`.
