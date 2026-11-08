# E23. Final-stack M2 and M4 closeout

Status: all seven items measured. The soak stopped on an availability event;
no replacement load was run. E23's owned processes and quiet request are gone.
The reviewer assigned lease availability to E24. Results count only after
independent reproduction.

| Item | Verdict |
| --- | --- |
| 1. vehicles | pass |
| 2. physics and approach | pass |
| 3. identical 1/2/4 tiles | pass |
| 4. proxy latency | not met, 7.31 ms against 5 ms on the conservative path |
| 5. additions tick profile | unproven, prewarm/control is already 7.6% of main Running time |
| 6. 30-minute soak | failed, availability event at about minute 23 |
| 7. performance release provenance | pass |

The selected stack is reactor-1 with FINAL-09 tiles, the E16 store and the
same sidecars as E20. Capacity stays 650 players, 2.0 times the matched plain
baseline of 325. The 3x target is not met and remains open. No candidate from
E21 or E22 replaces these bytes. No production source changes, commit, push
or upstream action are planned.

M2 retains its existing fixture geometry: 64-block tiles for vehicles and
512-block tiles for native physics. Performance cells use D9's 2,048-block
tiles. These are the same frozen executables, not different engine builds.

## Measurement contract

Each item has its own fixed plan below. Every attempt remains, including
startup errors, invalid noise rows, failures and unresolved windows. There
are no replacement runs chosen after seeing a result. A fixture repair needs
a separately named attempt and an explanation. The builder reports results;
the reviewer decides milestone acceptance.

Every load window creates its own `.local/quiet-request`, starts at one-minute
load at most 4 and releases after fixture cleanup. No outside process may
average above 0.5 core during either the measured interval or the whole cell.
Outside names become `outside`, with PIDs and CPU counters retained. Normal
windows are at most 25 minutes. The soak has one window of at most 40 minutes.
No other acknowledgement is required after the first host release.

Store traces and per-PID limiter signal-call brackets are captured by the
fixture, without changing frozen executable bytes or lease rules. A tile
that latches lease expiry ends that plan and is reported as an availability
event. No retry hides that event. Full native traces and screenshots stay
under `.local/evidence/E23/`. Raw JSON larger than 1 MB is compressed losslessly.
Worlds and logs are private disposable runs under `.local/runs/`.

## 1. M2 vehicles

Plan: boat, minecart, horse, in that order, one 100-crossing cell each. Use
E12's unchanged kinematic vehicle fixture through reactor-1. Check every
crossing generation, stable vehicle UUID, exactly one live vehicle, saved
player and entity files, all-tile reconciliation and mounted reconnect.
This is not vehicle-physics or multi-player vehicle acceptance.

| Row | Crossings | Stable UUID and generations | Live owners | Saved entity copies | Mounted recovery | Quiet seconds | Store batches |
| --- | ---: | --- | --- | ---: | --- | ---: | ---: |
| boat r1 | 100/100 | yes, 1 through 100 | one each | 0 | pass | 121.068 | 2,425 |
| minecart r1 | 100/100 | yes, 1 through 100 | one each | 0 | pass | 121.260 | 2,341 |
| horse r1 | 100/100 | yes, 1 through 100 | one each | 0 | pass | 121.076 | 2,404 |

Every row passed the noise and frozen-hash gates, retained only the current
owner's player file after all-tile reconciliation and recorded no lease
expiry or dropped limiter event. Horse r1's largest outside process averaged
0.06035 core. Each row includes saved-file and live-count evidence.

Verdict: passed in the builder's runs. Separate raw-row recomputation is in
`data/E23/vehicles-summary-v1.json`; reviewer reproduction remains required.

## 2. M2 approach and momentum

Plan: one full particle-off suite, then one full particle-on suite. Each has
56 native physics crossings, four modes and all 12 directed wall/corner
routes. Use checker v2, `scripts/e20-build10-check-v2`, without changing its
0.95 speed ratio or 0.1-block backward-correction lines. Retain unresolved
windows. Pair approach preparation timestamps with client wall crossing and
report their signed lead time for each crossing. Preparing before the wall
is distinct from freezing or activating before the wall.

| Row | Actual crossings | Resolved v2 windows | Sprint packet checks | Approach before wall | Smallest approach lead ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| particles off r1 | 56 | 56 | 14 | 56/56 | 151.674 |
| particles on r1 | 56 | 56 | 14 | 56/56 | 183.174 |

The target's real, held/unplaced session is sampled every 20 ms. The lead
uses the received status timestamp, so it is a lower bound on when target
preparation began. Wall time is interpolated between the unchanged client
movement packets. The target is already present before the wall in every
trial. Terrain readers are excluded from these real-session observations.

| Mode | Minimum speed ratio, off | Minimum speed ratio, on |
| --- | ---: | ---: |
| walk | 0.983913 | 0.966607 |
| sprint | 0.966458 | 0.965600 |
| sprint-jump | 0.996100 | 0.996570 |
| fly | 0.966304 | 0.966625 |

All 112 correction budgets resolve with maximum backward correction zero.
There are no unresolved timing windows, missing coverage or checker failures.
Both quiet/noise gates, store traces and frozen identities pass, with no
lease expiry. Collections took 306.962 and 307.394 seconds including cleanup.
The native movement code, v2 checker and its thresholds were not changed.

Verdict: passed in the builder's runs. Raw suites are under `data/E23/physics/`;
the checker output is `data/E23/physics-final-checker-v2.json`, and the
combined speed/approach result is `data/E23/physics-summary-v1.json`.

## 3. M4 identical per-tile load

Plan: 1, 2, 4 active tiles, in that order, two repeats each. Every active
tile gets exactly 162 players on the same seeded 128-block patrol square,
view/simulation distance 3, random tick speed 3 and 64 AI-enabled invulnerable
pigs. Totals are 162/324/648 players and 64/128/256 pigs. This equality-only
matrix has no crossing cohort. It is not a new capacity frontier or a full
D9 crossing comparison.

All cells use the same frozen proxy, store and active-tile sidecars. Production
readers stay enabled, with no eager/off override. The interior layout should
naturally have zero foreign readers; actual reader counts are reported.
Only the number of active tiles changes. Each cell has 30 seconds of warmup
and 60 measured seconds. The server fixture shares four cores and bots one
separate core. Report proxy CPU beside every tile's CPU, TPS, raw tick P99,
placed/held sessions, readers, chunks and pigs. Retain limiter pauses.

Values separated by `/` follow tile order a/b/c/d, with inactive tiles omitted.

| Row | Proxy cores | Tile cores | Tile TPS | Raw tick P99 ms |
| --- | ---: | --- | --- | --- |
| 1 tile r1 | 0.2440 | 0.4503 | 20.0073 | 18.140 |
| 1 tile r2 | 0.2214 | 0.4229 | 19.9926 | 17.836 |
| 2 tiles r1 | 0.4154 | 0.4120/0.4165 | 19.9978/19.9977 | 16.951/17.712 |
| 2 tiles r2 | 0.4226 | 0.4409/0.4358 | 20.0111/20.0097 | 18.505/18.130 |
| 4 tiles r1 | 0.7296 | 0.4554/0.4405/0.4581/0.4381 | 19.9980/19.9980/19.9980/19.9980 | 19.995/20.748/21.293/19.825 |
| 4 tiles r2 | 0.7172 | 0.4570/0.4472/0.4445/0.4540 | 20.0025/19.9870/20.0044/20.0043 | 20.349/19.889/19.897/21.013 |

Every tile has 162 placed players, zero held players, 64 pigs and 333 chunks
both before and after measurement. Foreign reader counts are zero because
all routes stay far inside their tiles, not because readers are disabled.
Sampled proxy thread counts are 341, 665 and 1,313 at 1, 2 and 4 tiles.
No raw tick sample dropped. Every row passes bot, raw tick, TPS, host-noise,
hash and lease-capture checks. There was no lease expiry.

Verdict: passed for identical per-tile gameplay work. This does not claim
bit-identical CPU or a new capacity frontier. All six rows and their complete
per-tile values are retained in `data/E23/identical-summary-v1.json`.

## 4. M4 proxy latency

Plan: one tap-free 650-player D9 control followed by one 650-player D9 cell
with partitioned external witnesses. Both have 30 seconds of warmup and 60
measured seconds, four server cores and one bot core. Partition forwarding
across independent processes rather than one saturated Python selector.
Select the same eight witnesses before startup. Match unchanged movement
payloads from front-read to backend-read, preserving all tails and counting
unmatched front packets as infinity. Report proxy CPU and thread counts,
per-tap CPU, forwarding queue high-water marks, internal forwarding histograms,
TPS, bot deadlines, activation counts and limiter pauses in both cells.

The external clock includes tap and TCP queues, so it is an upper bound on
the sampled movement path, not an all-packet wire measurement. Internal
histograms retain their narrower decoded-read-to-completed-write scope.
External P95 must be under 5 ms. Saturated taps, errors, packet loss or changed
proxy throughput make the latency result intrusive or unproven, not a pass.

| Row, 650 players | Proxy cores | Sampled maximum threads | Clientbound internal P95 ms | Serverbound internal P95 ms | Server STOP wall s | Bot STOP wall s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| tap-free control r1 | 1.08065 | 1,330 | 1.87 | 0.55 | 3.2422 | 0 |
| partitioned witness r1 | 1.13924 | 1,331 | 2.28 | 0.88 | 4.1801 | 0.1973 |

The external conservative movement-path P95 is **7.309333 ms**, above the
strict 5 ms line. It matches 9,626 of 9,628 front packets. Both unmatched
packets stay in the P95 denominator as infinity. No tail is trimmed and no
limiter pause is subtracted. This clock includes both taps and TCP scheduling;
it does not prove that proxy-only wire residence exceeds 5 ms.

All nine partitions have headroom: 0.0996 to 0.2037 core each. Their largest
forwarding queue is 46,242 bytes. All eight preselected witnesses appear in
the measured interval, with four interior tiles and two near-wall witnesses.
The ordinary serverbound forwarding count is 2,366,883 in the control and
2,366,828 with witnesses, a ratio of 0.9999768. Activations are 587 and 585.
Both rows retain all 650 players, pass D9/TPS/raw-tick/noise checks and record
no lease expiry. The witness row's 25 short bot STOP cycles cause no late or
failed bot rounds. No single-process observer saturation was observed.

Internal reader-reply P95 is 0.88 ms in the control and 1.22 ms with witnesses.
Generated unstamped clientbound writes remain separate: 157,176 and 156,898.
Read-to-activation-ack P95 is 47.45 and 97.75 ms, separate from ordinary
forwarding. Internal clocks exclude kernel queueing before the decoded read.
External wire evidence covers unchanged movement, not every protocol type.

Verdict: not met. The fixed conservative path measures 7.31 ms against 5 ms,
including limiter stops. The ordinary internal histograms pass,
but do not replace that external bound. Both rows remain in
`data/E23/latency-summary-v1.json`; no replacement run was added.

## 5. M4 busiest-tile tick profile

Plan: one 650-player D9 cell. Select the busiest tile from native CPU deltas
after warmup, then collect native thread counters and a Running-only CPU
profile. Keep traces local. Attribute cut writer, lease renewal, ownership
checks, digest and reader work separately where symbols allow it. Distinguish
off-tick worker CPU from tick-thread CPU and blocked stack occupancy from
Running weight. Digest may be unarmed; record that explicitly. Inlined or
mixed categories remain unattributed. Do not infer a precise fraction from
an incomplete sample. The combined additions line is strictly below 5% of
tick cost. A failed or unproven line is an allowed result.

The last five warmup seconds select tile d at 0.64164 native process core,
ahead of a 0.63763, c 0.53253 and b 0.52622. Its subsequent full-window
process average is 0.58156 core. Native matched thread counters give:

| Thread owner | Full-window cores |
| --- | ---: |
| main | 0.330205 |
| cut writer | 0.048272 |
| lease renewal | 0.000111 |

The native Time Profiler capture succeeds with no errors. It contains 5.688
Running CPU seconds, including 3.343 on main and 0.510 on the off-tick cut
writer. Waiting or blocked weights are excluded from the CPU denominator.

| Exclusive main Running category | CPU seconds | Percent of main |
| --- | ---: | ---: |
| between-tick pacing/control, including inlined warming | 0.253 | 7.568 |
| entity tracking, real players and readers mixed | 0.100 | 2.991 |
| named lease checks | 0.028 | 0.838 |
| other/unattributed main work | 2.962 | 88.603 |

Ownership checks can be inlined and reader work is mixed with real-player
tracking. Missing named frames do not mean zero cost. The full region digest
is unarmed, with zero digest records; the same raw tick recorder remains
enabled. Off-thread writer CPU is not a tick-stall percentage, and the mixed
between-tick control category is not an isolated additions fraction.

All 650 players remain online. The perturbed cell also passes its raw tick,
TPS and noise gates, with no dropped ticks or lease expiry. This is profile
evidence, not another capacity frontier repeat.

Verdict: unproven. Prewarm/control alone is 7.6% of main Running time; no valid
combined under-5% tick-cost bound is established. The numeric profile is
`data/E23/profile-final-r1-profile.json`.
Full native traces and exports stay in `.local/evidence/E23/profile-final-r1/`.

## 6. M4 30-minute crossing soak

Plan: one 650-player D9 soak with the same four-core server and one-core bot
budgets, continuous wall/corner crossings and 256 pigs. After normal join
and 30-second warmup, measure 30 minutes. Record per-production-process RSS
at minute 5 and minute 30, plus intermediate samples. Growth must be below
10% for every tile, proxy, store and sidecar. Record client/helper memory
separately. Drain raw ticks at least every 60 seconds without discarding any.
Check online counts, generation/owner transitions, frozen/acting session
states, inventory identities and named-item conservation. Zero duplicates
and zero owner violations are required. Lease expiry stops this plan.

Each of the 65 crossing actors receives one uniquely named cargo item during
the normal join placement, before warmup. The endpoint audit checkpoints and
restarts all tiles, then reads stopped-world player/entity files against the
current store owners and generations. Store entry anchors are not counted
as extra live copies. A temporary `caffeinate -i` assertion prevents idle sleep
for this run only; no persistent power setting changes.

The first retained session-ended proxy warning occurs 1,367.828637 seconds
after measurement start, about 22m48s. The controller captures failure about
34 seconds later. All five bot cohorts exit with client failures. Tile a's
latched local lease expiry is explicit at the subsequent cut request; expiry
of every other tile is not independently established.

The plan stops immediately after this event. Its one row remains failed in
`data/E23/soak-final-plan.json`. There is no retry. Twenty-two preceding
minute-spaced owner audits observe zero violations and no sampled generation
regression. Crossing query brackets remain unresolved rather than being
counted as violations. The native server logs retain 13,939 deduplicated
completed source releases, including join and warmup, not an exact measured
activation ledger.

The minute-30 endpoint does not exist. The last retained RSS observation is
at 1,367.130281 seconds, compared below with the 299.694493-second sample.
These are pre-failure diagnostics, not the requested minute-5 to minute-30
acceptance result.

| Process | About minute 5 MiB | Last retained MiB | Growth |
| --- | ---: | ---: | ---: |
| proxy | 592.969 | 691.953 | 16.69% |
| store | 95.141 | 186.031 | 95.53% |
| tile a | 472.906 | 488.281 | 3.25% |
| tile b | 435.438 | 448.406 | 2.98% |
| tile c | 458.031 | 462.516 | 0.98% |
| tile d | 460.438 | 469.297 | 1.92% |
| sidecar 0 | 2.844 | 2.906 | 2.20% |
| sidecar 1 | 2.797 | 2.891 | 3.35% |
| sidecar 2 | 2.828 | 2.844 | 0.55% |
| sidecar 3 | 2.859 | 2.859 | 0.00% |

### Availability trace and reviewer readback

The store trace retains 94,547 batches. The reviewer's readback finds about
64 batches/s steadily through the end, with a largest inter-batch gap of
283 ms. Store processing did not stall in that retained trace. The reviewer
also found that receipts are never pruned: store RSS rose about 95%, while
background compaction grew from roughly 120 to 952 ms. Those background
durations do not by themselves identify the cause of lease expiry.

The largest captured STOP-to-CONT signal-call bracket is 254.36 ms, on the
store. All limiter events are retained with zero drops. The largest
whole-host sampler spacing is 1.098 seconds, and outside-process noise passes
at a maximum 0.06447 core. These observations do not establish a host stall
or the lease-expiry cause. The frozen engine has no per-renewal tile trace.
E24 owns the separate availability investigation. E23 did not change store
or Horizon lease code.

### Failed-run retention and cleanup

Recovery teardown hit the expiry refusal before completing its cargo audit.
It also left the store and three tile processes alive. Their exact frozen
executable paths and unique E23 run directory were verified before sending
SIGTERM to those four owned PIDs. Every owned fixture PID is now gone.

The executed fixture also failed to serialize accumulated tick drains on this
exception path. That evidence gap is not repaired retroactively. No complete
soak tick, minute-30 memory, final cargo-conservation or recovery pass is claimed.
The run's small native failure evidence, memory samples, owner audits, store
trace and limiter signals remain; disposable worlds/logs were removed.

The exact executed fixture source is retained at
`data/E23/harnesses/soak-observe-executed-v1.py`, SHA-256
`905338f15099e91def369b4b5f017cc1cf0f3d6344ca9b5b96e43920e2850b0b`.
Only fixture teardown and partial-drain retention were fixed afterward.
Three resource-free long-run contracts pass, including refusal-path cleanup.
No load was rerun after that fix, and no frozen binary changed.

Verdict: failed on an availability event at about minute 23. Endpoint
conservation and minute-30 memory remain unverified. Readback is in
`data/E23/soak-availability-event-v1.json` and `data/E23/soak-summary-v2.json`;
the original failed row and both earlier derived summaries remain intact.

## 7. M4 release provenance

Plan: hash every capacity/latency executable before and after every cell.
Match each hash to a retained successful `cargo build --release` record.
Record manifests, patch identities and the frozen launcher too. A file path
containing `release` is not proof of its build profile. No rebuild is planned.

The reviewer confirmed the milestone scope: tiles, proxy, store, sidecars
and load bots, whose CPU, TPS, tick or latency figures enter performance
results. E14's pinned native physics client and `horizon-ctl` are correctness
fixtures built with `--profile fast`. Their speed ratios compare against
their own no-wall baseline. No capacity or proxy-latency claim uses those
fixture binaries, and they are not relabelled release builds.

Verdict: pass for the performance scope. All six selected
executables match successful retained release-build records and current
SHA-256 readback in `data/E23/preregister-v1.json`. Every completed or aborted
window retains matching before/after identities. The native correctness
client and controller remain at their original checker-pinned hashes.

## Verification and host handoff

Seventeen E23 static checks, three long-run contracts and twelve unchanged
E18 measurement-tool tests pass. The owner-duplication control deliberately
raises a violation and is caught by its test. The long-run tests prove drain
concatenation, drop-count retention and cleanup after a refused checkpoint.
No Rust code was edited or compiled by E23. No root Rust gate is claimed.

The final evidence audit checks frozen identities, outside-name redaction,
executed fixture copies, writing rules and lossless compression. Large JSON
is retained as `.json.gz`; native store CSV and local profile traces keep
their compressed native formats. Screenshots and environment traces remain
under `.local/evidence/E23/`. The audit is `data/E23/final-integrity-v1.json`.

Plain `scripts/clean-builds`, never `--all`, completed and freed 0 MiB. Root,
Horizon and Meridian targets were below their caps at 38, 2,017 and 683 MiB.
The project measured 8,453 MiB at cleanup, an end sample rather than a peak.
No owned load or quiet request remains. The host is released to the reviewer
and E24. No commit, push, upstream action or milestone checkbox change was made.

## Frozen identities and reproduction

The fixture plan and hash audit will be retained under
`docs/experiments/data/E23/`. Exact commands are added as their entry points
are statically checked. No milestone box is ticked by this report.

| Frozen artifact in `.local/bin/meridian-e18/reactor-1/` | SHA-256 |
| --- | --- |
| `horizon-client`, FINAL-09 | `b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d` |
| `meridian` | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| `goliath-store` | `6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af` |
| `meridian-sidecar` | `c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b` |
| `horizon-bots` | `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d` |
| `e4-client` | `e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025` |

Run from the repository root with fresh names. `--host-released` requires the
same explicit host release used for this campaign. Each command creates and
releases its own request. Never pre-create the quiet marker.

```sh
python3 -B scripts/e23-tests
python3 -B scripts/e23-prepare --name repro-registration
python3 -B scripts/e23-window --name repro-vehicle-boat-r1 --host-released -- python3 -B scripts/e23-vehicles --name repro-vehicle-boat-r1 --kind boat
python3 -B scripts/e23-window --name repro-vehicle-minecart-r1 --host-released -- python3 -B scripts/e23-vehicles --name repro-vehicle-minecart-r1 --kind minecart
python3 -B scripts/e23-window --name repro-vehicle-horse-r1 --host-released -- python3 -B scripts/e23-vehicles --name repro-vehicle-horse-r1 --kind horse
python3 -B scripts/e23-results vehicles --prefix repro-vehicle --name repro-vehicle-summary
python3 -B scripts/e23-campaign physics --name repro-physics --host-released
python3 -B scripts/e23-results physics --prefix repro-physics --name repro-physics-summary
python3 -B scripts/e23-campaign identical --name repro-identical --host-released
python3 -B scripts/e23-results identical --prefix repro-identical --name repro-identical-summary
python3 -B scripts/e23-campaign latency --name repro-latency --host-released
python3 -B scripts/e23-results latency --prefix repro-latency --name repro-latency-summary
python3 -B scripts/e23-campaign profile --name repro-profile --host-released
caffeinate -i python3 -B scripts/e23-campaign soak --name repro-soak --host-released
python3 -B scripts/e23-soak-results --run repro-soak-r1 --name repro-soak-summary
python3 -B scripts/e23-audit --name repro-integrity --compress
scripts/clean-builds
```
