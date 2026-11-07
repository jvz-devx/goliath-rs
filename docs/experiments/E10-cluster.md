# E10. Integrated cluster

Implementation evidence. Review is separate. Nothing was committed,
pushed, or sent upstream. No Folia, Paper, GPL source or excluded research was read.

## Current verdict

The real cut, fenced player store, tile ownership and proxy are wired together.
The combined binary passed 1,000 consecutive crossings, four-tile clean recovery,
named-item transfer fixture, 200 kill trials and all 22 east-wall breach fixtures.
Twenty-five walking clients completed 140 cuts with zero refusals or disconnects.

Every crossing had a visible store CAS and an unchanged inventory. There were
zero disconnects and zero client position corrections. Input gap P50/P95/P99 was
75.702 / 124.001 / 133.136 ms, including the cut and CAS. This passes the numeric
M2 gap line. It does not close the remaining M2 terrain, physics and vehicle work.

Work paused for E11's quiet capacity window at 21:29:30 local time on 2026-11-07
and resumed after its marker disappeared. No E10 builds or other E10 fixtures ran
during the 1,000-crossing timing measurement. The corrected normal-terrain
walking/freeze check missed the 40 ms durable-cut budget. It completed 140 cuts
with zero refusals and exercised 397 proto snapshots and 80 unload snapshots.
Tick-stall P95 passed at 1.173 ms, but request-to-durable P95 was 48.730 ms.
The corrected binary passed the repeated 1,000-crossing proof, 200 kills,
22 breach fixtures and scoped verification. The 40 ms normal-generation cut
budget remains a failure. M1 and M2 are not fully closed.

Chat, a cluster tab list and scoreboard transfer are not implemented. Full terrain
composition past a wall, physical momentum, a stock client and vehicle handover
remain outside this pass. Weather is a stored startup setting, not a live cluster
weather simulation.

## Architecture as built

Four Horizon processes share coordinates and seed but have separate working
worlds and cut logs. E7's owned areas are half-open squares. The proxy chooses
the tile using client positions. Production edge defaults to 2,048 blocks;
the crossing and item fixtures use 64. The walking fixture uses 512.

Each tile launch claims a fresh process fence from the real store before starting
Horizon. The launcher installs that token as the tile log's local append fence
and passes the store address, tile ID and token to Horizon. Startup replays the
tile cut, reconciles player generations against the store and completes the cold
cut before the proxy starts accepting players. New logins create a full-UUID
record at generation zero. Reconnecting players route to their store owner.

The crossing order is:

```text
source tile-cut <fence> <uuid>
  freeze, capture one coherent world/player image
  encode, append, full flush, durable receipt
proxy fenced store CAS
  source generation and both process fences checked
  next generation and complete activation blob made durable
target activation
  exact committed record and placement checked against the store
  place player, without a second durable player-file commit
proxy swaps the view and replays ordered input
source releases the departed player
```

Meridian uses a path dependency on `goliath-store`. Horizon also uses that client
for registration, activation checks and recovery. Both additions live in patches
against the pinned upstream commits. The ordinary sidecar still handles the
existing held-login and release requests. The cluster adapter uses Horizon's
existing authenticated loopback control endpoint for the combined cut request.

The store's E5/E9 operations retain their wire tags. New operations carry full
128-bit UUIDs, a bounded listing of 256 UUIDs per page, and the cluster clock.
The simulator's small integer player IDs are not used as truncated UUIDs.
UUID records retain the same generation/fence kernel and journal barrier ordering.

## Dynamic cuts

The stable-player and stable-chunk-footprint restrictions are removed.

- Unloads retain immutable full chunk images with current block entities,
  scheduled ticks and entity records until the next cut.
- Accepted generation retains proto snapshots and finished nonresident columns.
  Jobs still running have not entered simulation and belong to a later cut.
- A cut merges unloads in order. A current resident supersedes its older unload.
- A disconnect retains the player's final compound, statistics and advancements.
  A later resident image of that UUID supersedes those earlier records.
- Pending loaded entity tags enter the game before capture, so a cut cannot omit
  entities between their load and their first normal tick.
- Dirty resident COW chunks, block-entity revisions and metadata overlays retain
  E6's tracking. Worker append, full flush, compaction and prefix recovery are
  unchanged.

Deliberate live entities outside every resident chunk still fail closed. Native
mods still lack a snapshot contract. IO failures still poison the writer. Those
are not the removed restart-on-normal-streaming restrictions.

The cold full-save bootstrap remains. `goliath-cluster` runs it before opening the
proxy, not during a player's crossing. Calling the patched control API outside
this startup workflow does not provide that guarantee.

E7 originally corrected a walking client back inside before E4's trigger margin
could be reached. Cluster mode now clamps the server position before collision
checks without sending that boundary correction. Simulation remains inside.
Ordinary E7 mode keeps its correction behavior. The proxy carries the moving
client across the wall; this is not a claim about a physics client's momentum.

## Recovery and authority

Startup holds the world session lock while it restores the durable cut and
reconciles with the store. A departed player is removed from that tile's files.
A matching-generation local player is kept, including progress newer than its
store entry. A newer store entry is materialised with its position, motion,
rotation, statistics and advancements. A local generation newer than the store
fails closed rather than being overwritten.

Target import validates the exact committed player and placement fields before
placing. In cluster mode its old durable per-player file writes are redundant
and skipped. A cut containing that player anchors subsequent gameplay; otherwise
startup materialises the store entry. Ordinary imports retain their durable
file-write path.

The proxy resolves an uncertain CAS before aborting a source. If authority may
have moved, it closes the client instead of resuming an old writer. A cluster
freeze that expires also disconnects rather than automatically resuming. Random
tile kills during crossings, partitions and process replacement while an old
tile keeps simulating are not proven here. They remain M3 work.

The one-crossing `entry-recovery-final` fixture deliberately omits the target's final
tile cut before shutdown. Recovery then restores its older cut and materialises
the committed generation-one entry from the store. Inventory matched, all four
world markers remained, and reconnect routing selected tile b.

The separate `recovery-files-final` route checks the restored files directly.
Only tile a, the final owner after 14 crossings, kept the player's data file.
Other tiles had neither the departed data file nor its fallback. The owner's
file held generation 14 and the known inventory checksum. This checks the
generation filter rather than inferring it only from successful reconnects.

## Global state and activation requests

The store holds one wall-clock anchor, game/day ticks, rain and thunder flags.
The proxy rewrites both relayed time packets and swap-delta time packets from
that anchor. The 1,000-crossing run observed 1,635 client clock packets with
zero backward steps. This is protocol evidence, not rendered sky acceptance.
Weather rewriting uses the startup record. Live weather updates and cycling are
not implemented.

The proxy presents a common sky. Tile simulation clocks still advance locally;
their time-dependent gameplay is not driven from the stored clock in this pass.

Statistics and advancements already carried by Horizon's transfer format now
travel in the authoritative store blob and recovery path. A non-default
statistics/advancement gameplay fixture has not been run.

Positioned held logins prepare the target view at the approach point without
importing or placing a player. Control replies include queued-wait, application
time and simulation tick. The pacing control prelude removes the source's
full-tick dispatch wait. E8's bounded ordered input replay remains after
activation. An ordered input batch applied inside the activation tick is not
implemented. Terrain outside a tile is still absent from the composed view.

## Results so far

Milliseconds. Percentiles are nearest rank. A 14-sample P99 is just the maximum.

| Run | Count | P50 | P95 | P99 | Other checks |
| --- | ---: | ---: | ---: | ---: | --- |
| Integrated pilot input gap | 14 | 80.105 | 131.868 | 131.868 | 14 visible CAS, zero corrections |
| Fresh-source build input gap | 14 | 56.473 | 128.172 | 128.172 | 14 visible CAS, all four tiles restarted |
| Final input gap, cut and CAS included | 1,000 | 75.702 | 124.001 | 133.136 | 1,000 visible CAS, zero corrections |
| Trigger to arrival message | 1,000 | 178.710 | 243.326 | 260.307 | Includes target preparation after the trigger |
| Trigger to first target input | 1,000 | 211.143 | 288.024 | 302.244 | Different endpoints from input gap |
| Walking cut request to durable reply | 140 | 10.973 | 15.224 | 18.580 | 25 clients, zero refused cuts |
| Walking cut tick stall | 140 | 0.373 | 0.650 | 1.628 | Below the 5 ms P95 line |
| Walking cut capture body | 140 | 0.340 | 0.623 | 1.358 | Excludes worker encoding and IO |
| Normal-terrain freeze/cut | 140 | 21.145 | 48.730 | 54.029 | Zero refusals; misses 40 ms P95 |
| Normal-terrain tick stall | 140 | 0.635 | 1.173 | 1.417 | Passes 5 ms P95 |

All eight directed cardinal borders and all four diagonal directions appear in
the final run. The route repeats E4's 14-leg itinerary. Every record's generation
is its crossing index plus one. The independent analyser checks the complete
owner chain, known inventory checksum, accepted target tile, cut sequence,
observed CAS, phase order and percentile arithmetic. No sample is trimmed.

| Crossing phase, ms | P50 | P95 | P99 |
| --- | ---: | ---: | ---: |
| Freeze/cut request to durable reply | 8.544 | 12.981 | 14.165 |
| Fenced store CAS | 4.446 | 5.259 | 5.834 |
| Target activation request/reply | 3.523 | 4.353 | 4.515 |
| Source cut tick stall | 0.602 | 0.741 | 0.824 |
| Source cut capture body | 0.206 | 0.268 | 0.306 |
| Worker encode | 1.900 | 2.687 | 3.752 |
| Append and full flush | 4.209 | 8.586 | 9.491 |
| Target control queued wait | 0.739 | 1.424 | 1.491 |
| Target activation application | 2.397 | 2.632 | 2.751 |

Component percentiles are not added to manufacture an end-to-end percentile.
Input gap uses E4/E8's first accepted target input minus last accepted source
input. The source can accept clamped movement while the target prepares, so
this is not the whole time a player waits to move into another tile. Trigger
latencies above make that distinction visible. Physical momentum and action
latency while the client is past the source wall remain unverified.

The final run's one-minute load average ranged from 0.844 to 4.321. This is a
loopback observation on the shared Mac, not a CPU-limited capacity benchmark.

The walking clients moved from x=16..20 to x=360..364 at 5.6 blocks/s, loading
fresh flat terrain. Cuts ran at 2 Hz for 70 seconds. Both endpoint statuses had
25 players. Resident chunks went from 49 to 116. The cut log accepted 80 unload
snapshots. There were zero refused cuts and zero client disconnects. This is
flat-world streaming evidence. The run overlapped other safety
fixtures and part of scoped verification on the shared machine, not an isolated
performance campaign.

The corrected normal-terrain run uses 25 kinematic creative clients moved to
y=302 during setup, so terrain cannot turn a walking test into a standing one.
All reached x=360..364. Both endpoint statuses had 25 players and resident chunks
went from 49 to 116. It logged 397 accepted proto snapshots and 80 unload
snapshots across 140 freeze/cut requests at 2 Hz, with zero refusals and
disconnects. No other E10 build or fixture ran during its measurement.
This is not a physics-client test, and it does not repeat E6's full chest-churn
and 64-pig performance fixture.

Normal-run worker encode P50/P95/P99 was 9.718 / 37.684 / 43.117 ms;
append/full-flush was 8.341 / 10.339 / 10.742 ms. The tick side fits, but encoding
new terrain and proto state misses the frozen-window share at P95.
Pre-encoding immutable generation state between cuts is a possible next step.
No durability barrier was weakened to improve these numbers.

Tick stall uses E6's control-handler capture interval. Proto snapshots made when
generation results enter the world happen during ordinary ticks, outside that
interval. Their full added tick cost has not been profiled here. The normal
request maximum was 60.540 ms, with 22 of 140 requests at or above 40 ms.

The named-item fixture moved seven `E10Chest-Alpha` diamonds and eleven
`E10Chest-Beta` diamonds out of a chest with real menu clicks. The giver dropped
Alpha and the receiver picked it up. Both players crossed, then every tile
restarted cleanly. Recovered player counts were exactly 7 and 11. The chest
remained empty. No loose item entity remained before or after restart.

The final combined Horizon binary, SHA-256
`5c627257191bd374e8e7417dc39b56ab683f6f674c3625be99cb4229a6595337`,
passed 200/200 E6 kill trials with zero mixed images. All thirteen E6 gates,
including mid-append and compaction, remain covered by that fixture. These are
local coherent-cut kills, not tile kills during distributed crossings.

The same binary passed 22/22 east-wall mechanics, each with a positive control,
and zero changed exterior records. This is one direction per E7 mechanic, not a
repeat of all 176 original directions/mechanics. The fixture's `complete_matrix`
field refers to the deliberately reduced one-direction matrix.

## Build and verification

The fresh build exported pinned Horizon and Meridian sources under one temporary
`.local/runs/E10-build-*` layout, applied the recorded patch stacks, copied the
current internal crates for the relative path dependencies, and built all three
workspaces. It did not reuse a target directory from another workspace.

| Fresh build | Seconds | Result |
| --- | ---: | --- |
| Store, release | 0.787 | Passed |
| Combined Horizon, fast | 156.637 | Passed |
| Meridian, sidecar and client, fast | 22.044 | Passed |

A first fresh build continued into the real four-tile, 14-crossing restart proof
and passed. The later fresh build above includes the test-client drop repair and
proto forwarding fix. It continued into the normal-terrain streaming/freeze test.
Its exact retained binaries are used by the final crossing and recovery proofs.
Temporary source and build directories were deleted after each build.

Root formatting, strict normal/all-feature Clippy and 35 normal/all-feature tests
passed. Combined Horizon scoped strict Clippy passed. Its six cut-store tests
passed, including every-byte torn tails, corruption, stale writers, compaction,
grouped records and metadata overlays. Two tile-geometry tests and the combined
source's solid-unknown-read test passed. Meridian scoped strict Clippy passed;
the config, ghost, handoff and proxy suites passed. The existing listener-burst
test was explicitly skipped, matching E8. No full Horizon gameplay or vanilla
parity suite is claimed.

Eight offline evidence-checker tests passed, including rejection of missing
crossings, wrong CAS generations, inventory changes, incorrect timing arithmetic,
reordered phases and backward clock evidence. A correction over 0.1 blocks cannot
silently pass the correction budget. Final patch application and byte-equality
checks passed for both upstream trees. `git diff --check` passed.

Builds use `CARGO_INCREMENTAL=0`, one target directory per workspace and the fast
profile for Horizon and Meridian. This is not M4 release-capacity evidence.
Retained executables are under `.local/bin/goliath-e10/`. Worlds, logs, homes and
XDG directories are private run folders under `.local/runs/` and are removed.
Prepared 26.3 resources are opened read-only. No launcher management command ran.

## Commands

From the repository root:

```sh
python3 -B scripts/e10-patch
python3 -B scripts/goliath-cluster proof --build --fresh --crossings 14 --name fresh-smoke
python3 -B scripts/goliath-cluster proof --crossings 14 --name pilot-3
python3 -B scripts/goliath-cluster proof --crossings 1000 --name crossings-final
python3 -B scripts/e10-analyse
python3 -B scripts/e10-runner-test
python3 -B scripts/goliath-cluster proof --crossings 1 --materialize --name entry-recovery-final
python3 -B scripts/goliath-cluster proof --crossings 14 --name recovery-files-final
python3 -B scripts/goliath-cluster items --name items-final
python3 -B scripts/goliath-cluster walking --seconds 70 --name walking-25-final
python3 -B scripts/goliath-cluster walking --build --fresh --normal --seconds 70 --name walking-normal-final
python3 -B scripts/e10-regressions kills --kills 200
python3 -B scripts/e10-regressions breaches
python3 -B scripts/e10-verify
cargo fmt --all --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test --workspace --all-features
```

Final cleanup:

```sh
scripts/clean-builds
```

`scripts/goliath-cluster start` stays in the foreground and reports the proxy
port and state file. `scripts/goliath-cluster stop --state PATH` asks that owner
to stop. Start mode makes background cuts every five seconds. Production edge
is 2,048 by default, but the supplied launch settings remain an offline creative
flat-world experiment, not a public-server deployment configuration.

## Failures kept

- `pilot-1.json` reached no crossing because the E7 boundary correction prevented
  the client from reaching the proxy trigger.
- `pilot-2.json` exposed the earlier collision rejection before the later clamp.
  Moving the cluster clamp before collision checks fixed this second path.
- `items-1.json` used the wrong 26.3 player-action ID. The chest take succeeded,
  but no drop occurred. The corrected client uses action 4.
- `walking-25.json` sent its setup path through the deliberately placed world
  marker at 8,8. The corrected fixture takes two waypoints around that block.
- `walking-normal-before-forwarding.json` is rejected despite its original
  runner's completion flag. Clients stopped near x=109, and no proto or unload
  snapshots reached the log. `Persistent` inherited the generator trait's default
  `saved_proto`, so it discarded the cut tracker's snapshots. That method now
  forwards to the generator. The runner requires completed travel and nonzero
  proto/unload capture. The corrected run's 48.730 ms P95 miss remains a miss.
- Initial builds caught the missing queue timestamp in benchmark/UI control
  request constructors, a missing map import and an undefined Meridian fast
  profile when its environment override was omitted. All were fixed.

Successful replacement runs have distinct names. No failed sample is pooled
into a successful percentile table. Small raw results are in `data/E10/`.

## M1 and M2 item verdicts

No milestone boxes are ticked by this implementation report.

| Item | E10 verdict |
| --- | --- |
| M1 tile size/global-state/dimension decision | Existing D8 remains. Several global-state services below remain unbuilt. |
| M1 coherent tile cut under load | Dynamic normal/flat streaming exercised with zero refusals, plus 200 final-binary local kills. Normal frozen-window P95 misses 40 ms. |
| M1 refuses foreign simulation/write | Combined binary passed one direction of all 22 E7 mechanics. |
| M1 unknown chunks not air | Combined-source solid-unknown-read regression passed. |
| M1 automatic wall crossing | Integrated 1,000-crossing route passed. |
| M1 optional visible wall | E4/E8 implementation retained, not rerun here. |
| M1 shared fenced player store | Wired and exercised on every integrated crossing. |
| M1 1,000 walking crossings | Passed, with zero disconnects, inventory changes or corrections and every CAS visible. |
| M1 four-tile restart | Passed world markers, player inventories, departed filtering and newer-entry materialisation. |
| M1 fresh-checkout build | Replacement integrated fresh-source build path ran end to end. |
| M2 read-only terrain past wall | Not implemented. Positioned held view is not terrain composition. |
| M2 approach/momentum/no correction | Approach and zero corrections over 1,000 crossings observed. Physics momentum unverified. |
| M2 gap P95 <150/P99 <300 over 1,000 | Passed in the flat E4/E8 accepted-input fixture, P95 124.001 ms and P99 133.136 ms. Not a normal-terrain end-to-end claim. |
| M2 real physics client | Not run. |
| M2 stock client | Not run; maintainer acceptance remains required. |
| M2 vehicle handover | Not implemented. |
| M2 store group-commit P95 at 64 clients | E9 batching retained. No new 64-client acceptance measurement. |

Remaining explicit omissions are proxy-owned chat, the cluster tab list,
scoreboard transfer, live weather updates, ordered initial input inside the
activation tick and complete neighboring terrain. Statistics and advancements
are wired but lack a non-default gameplay fixture. Random crossing kills,
partitions, disk loss, power cuts, public authentication and a production server
configuration are not established by these tests.

## Disk and cleanup

Final `scripts/clean-builds`, without `--all`, exited 0. All target directories
were under its cap: root 55 MiB, Horizon 2,212 MiB, Meridian 500 MiB and the
other agent's Horizon-tiles 2,480 MiB. It freed zero MiB and reported the shared
project at 7,949 MiB. This is an end sample, not a continuously measured peak.

Every E10 world/log/home directory and both fresh-build staging directories were
removed after their run. Two generated Python cache files were removed too.
Source and retained binaries were not deleted. No E10 engine, proxy, store or
Cargo process remained. Raw E10 evidence totals 3,528 KiB. No result contains an
actual home path. Final patch byte-equality checks, evidence-checker tests and
`git diff --check` passed before handoff. No commit or push was made.

## Independent verification

The reviewer reran the cluster checks on 2026-11-07 with the retained binaries, at low load, then ran the fresh-checkout build separately.

| Check | E10 run | Reproduction |
| --- | ---: | ---: |
| Crossings completed, inventory equal, disconnects | 1,000, 1,000, 0 | 1,000, 1,000, 0 |
| Input gap P50 / P95 / P99, ms, cut and CAS included | 75.7 / 124.0 / 133.1 | 90.3 / 109.6 / 143.5 |
| Trigger to first target input P50 / P95 / P99, ms | 211.1 / 288.0 / 302.2 | 211.6 / 291.3 / 305.6 |
| Store generation equals crossing index plus one | all | all |
| Largest correction, blocks | 0 | 0 |
| Named items recovered after restart, Alpha / Beta | 7 / 11 | 7 / 11 |
| Normal-terrain walking cut P95, ms, refused cuts | 48.7, 0 | 45.6, 0 |
| Kill trials, mixed images | 200 of 200, 0 | 200 of 200, 0 |
| East-wall breaches blocked, positive controls | 22, 22 | 22, 22 |
| Fresh-checkout build plus 14 crossings | passed | passed in 202 s |

What the input gap does and doesn't measure: in cluster mode the source tile keeps accepting a crossing player's movement, clamped at the wall, while the target prepares. Those inputs count as acted on, which matches the metric's definition since E2: last input acted on by the source, first by the target. The player is held at the wall for that time, though, so trigger to first target input is about 290 ms at P95. A kinematic client saw no correction. A physics client, M2's next item, has to show whether a real player notices.

The normal-terrain cut still misses its 40 ms share. The crossing gap passes with the cut inside it, because the crossing runs on flat terrain with no new generation in flight.
