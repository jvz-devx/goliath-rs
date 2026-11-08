# E22. Shared terrain views

Verdict: hold, unqualified for promotion. Shared-4 is implemented, frozen and
release-verified. Its two valid D9 repeats save 0.030 and 0.216 core, so the
required 0.10-core saving is not reproduced. Both fixed contained repeats
completed too. No resampling or averaging changes that result. Full shared-4
M2 is skipped under the agreed condition; no further optimization round ran.

Horizon patch 11 passed release gates and legacy terrain/border checks. The
current Meridian stack passes 114 handoff and 566 proxy tests and a focused
real-client cache-eviction/return/flight probe. Earlier shared-1 and shared-3
builds have known terrain defects and cannot qualify the corrected build.
All failed and superseded evidence remains. The observed control-arm lease
availability event has unknown cause and stays reported; it did not recur in
the final eight-cell comparison. No milestone or independent acceptance is
claimed. Nothing was committed, pushed or sent upstream.

## Final shared-4 result

The completed ledger is `data/E22/shared4-traced-fixed-pairs.json`; the
hash-checked derivation is `data/E22/shared4-results.json`. No extra repeat,
resampling or averaging was used. The code and bounds are implemented, but
the CPU-saving criterion is not met in both repeats. The reviewer requested
handoff here, with no further optimization round. Full shared-4 M2 and checker
v2 are therefore not run; shared-3's earlier M2 results cannot qualify it.

| Layout | Repeat | Build | Proxy cores | Four tiles | Total server cores | Activations |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| D9 | 1 | current-1 | 0.891576 | 2.082008 | 3.004019 | 587 |
| D9 | 1 | shared-4 | 0.962266 | 1.986534 | 2.974399 | 585 |
| D9 | 2 | current-1 | 0.857975 | 2.034988 | 2.920742 | 586 |
| D9 | 2 | shared-4 | 0.800343 | 1.877836 | 2.704629 | 587 |
| Contained | 1 | current-1 | 0.689366 | 1.703491 | 2.400827 | 0 |
| Contained | 1 | shared-4 | 0.666369 | 1.697148 | 2.373150 | 0 |
| Contained | 2 | current-1 | 0.685166 | 1.733008 | 2.427806 | 0 |
| Contained | 2 | shared-4 | 0.648384 | 1.624614 | 2.282298 | 0 |

D9 total savings are 0.029620 and 0.216112 core. Proxy deltas are -0.070690
and 0.057632; tile savings are 0.095474 and 0.157152. Contained total savings
are 0.027676 and 0.145508, with zero handovers and the unchanged placement
contract. Readers are 65 per tile in the control and one per tile in shared-4.
The minimum tile TPS is 19.9759, maximum raw tick P99 is 39.6089 ms and no raw
ticks drop. All 650 bots remain online without lateness. Every row passes the
start-load and outside-noise gates.

Cache high-water marks beside per-tile CPU:

| Layout | Repeat | Tile | Shared tile cores | Peak source columns | Peak charged bytes |
| --- | ---: | --- | ---: | ---: | ---: |
| D9 | 1 | a | 0.517287 | 35 | 256380 |
| D9 | 1 | b | 0.500498 | 35 | 256307 |
| D9 | 1 | c | 0.481216 | 35 | 256380 |
| D9 | 1 | d | 0.487533 | 35 | 256380 |
| D9 | 2 | a | 0.479607 | 35 | 256380 |
| D9 | 2 | b | 0.465799 | 35 | 256380 |
| D9 | 2 | c | 0.468960 | 35 | 256380 |
| D9 | 2 | d | 0.463470 | 35 | 256380 |
| Contained | 1 | a | 0.605911 | 48 | 351506 |
| Contained | 1 | b | 0.369725 | 34 | 248984 |
| Contained | 1 | c | 0.359760 | 24 | 175754 |
| Contained | 1 | d | 0.361753 | 34 | 249057 |
| Contained | 2 | a | 0.577301 | 48 | 351579 |
| Contained | 2 | b | 0.351097 | 34 | 248984 |
| Contained | 2 | c | 0.357076 | 24 | 175754 |
| Contained | 2 | d | 0.339139 | 34 | 248984 |

Peaks include warm-up and charge payload plus 64 bytes per retained packet,
not process RSS, map metadata or client output queues. Each tile has one
group, so these are exact group/per-tile peaks. All measured request, deferral,
reset, restore and recovery-failure counters remain zero. Forced recovery is
covered by the deterministic-clock protocol tests, not inferred from those
zero counters. Hard limits remain 4,096 source columns, 64 MiB bases,
16 MiB journals and the documented membership bounds.

All eight store/signal captures are complete, with no dropped events and no
lease expiry. The largest store barrier is 17.7050 ms; the largest observed
store STOP call-bracket upper bound is 13.8941 ms. The earlier simultaneous
four-tile control expiry is still an observed availability event with unknown
cause, not erased by these successful cells. No lease duration, margin, budget
or frozen engine/store byte changed.

## All six D9 pairs

All values are measured CPU cores. Each current/shared cell uses the same
650-player D9 geometry, four-server and one-bot core caps, 30-second warm-up
and 60-second measurement. Only the proxy differs within a pair. Shared-1
predates store tracing; shared-3 and shared-4 enable the existing trace in both
arms. Cross-build absolute differences therefore do not isolate one fix.

| Build | Repeat | Control proxy / tiles / total | Shared proxy / tiles / total | Total saved | Qualification |
| --- | ---: | --- | --- | ---: | --- |
| shared-1 | 1 | 1.009959 / 2.129696 / 3.150799 | 0.911391 / 1.905953 / 2.828156 | 0.322644 | BUGGY, not qualifying |
| shared-1 | 2 | 0.910442 / 2.112945 / 3.034198 | 0.864536 / 1.990578 / 2.866092 | 0.168105 | BUGGY, not qualifying |
| shared-3 | 1 | 0.897918 / 2.124530 / 3.051060 | 0.920238 / 1.906516 / 2.854866 | 0.196194 | BUGGY, not qualifying |
| shared-3 | 2 | 0.897895 / 2.067588 / 2.995591 | 0.806398 / 1.892574 / 2.727083 | 0.268508 | BUGGY, not qualifying |
| shared-4 | 1 | 0.891576 / 2.082008 / 3.004019 | 0.962266 / 1.986534 / 2.974399 | 0.029620 | Below 0.10 |
| shared-4 | 2 | 0.857975 / 2.034988 / 2.920742 | 0.800343 / 1.877836 / 2.704629 | 0.216112 | One passing repeat only |

Shared-1 has the source-cache leave/re-enter race. Shared-3 has the client-cache
presence hole exposed by native physics. Their larger savings cannot qualify
shared-4. Both shared-4 rows pass tick, bot, budget and noise gates, but one
does not save at least 0.10 core. No mean, selected repeat or resampling is
used to change that result. The build remains hold/unqualified, with no full
shared-4 M2 campaign and no further optimization round.

The unchanged control itself ranges from 2.920742 to 3.150799 cores at the
same player load, about 0.230 core of observed spread. With only two pairs,
savings below about 0.15 core cannot be resolved reliably on this host. This
is a finding about the method, not a formal confidence interval and not a
reason to change the preregistered rule after seeing the results.

## Shared-3 frozen-byte CPU comparison, before the native presence fix

`data/E22/shared3-traced-fixed-pairs.json` is the completed fixed ledger.
`data/E22/shared3-results.json` is a hash-checked derivation, not rewritten raw
evidence. Both arms use frozen patch 11, the same store, sidecars, bots and
launcher. Only Meridian differs. Existing store tracing is enabled in both
arms and remains inside measured store CPU. D9 has the unchanged four-server
and one-bot core caps; the contained reader control is uncapped as in E21.

| Layout | Repeat | Proxy build | Proxy cores | Four tiles | Total server cores | Activations |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| D9 | 1 | current-1 | 0.897918 | 2.124530 | 3.051060 | 587 |
| D9 | 1 | shared-3 | 0.920238 | 1.906516 | 2.854866 | 585 |
| D9 | 2 | current-1 | 0.897895 | 2.067588 | 2.995591 | 585 |
| D9 | 2 | shared-3 | 0.806398 | 1.892574 | 2.727083 | 587 |
| Contained | 1 | current-1 | 0.676887 | 1.687901 | 2.373424 | 0 |
| Contained | 1 | shared-3 | 0.639063 | 1.620908 | 2.269271 | 0 |
| Contained | 2 | current-1 | 0.666299 | 1.694978 | 2.370577 | 0 |
| Contained | 2 | shared-3 | 0.655750 | 1.675584 | 2.339971 | 0 |

D9 total savings are 0.196194 and 0.268508 core, both above the reviewer's
0.10-core threshold. Tile savings are 0.218015 and 0.175014; proxy savings are
-0.022319 and 0.091497. The first proxy delta is retained as a regression in
that component, not hidden behind the total. The contained controls save only
0.104152 and 0.030606 core. They do not reproduce the 0.15-to-0.30 planning
hypothesis. The moving D9 and static contained cases have different costs;
neither is extrapolated to 975 players or a new frontier.

All eight rows pass start-load and outside-noise rules, retain all 650 bots
without lateness, and have no raw tick drops. The minimum tile TPS is 19.9812
and the largest raw tick P99 is 38.3751 ms. D9 activation counts remain close
in both arms. Both contained repeats retain E21's placed-player contract and
zero handovers. Reader sessions fall from 65 to one on every tile.

Per-tile CPU and source-cache high-water marks:

| Layout | Repeat | Tile | Current cores | Shared cores | Peak source columns | Peak charged bytes |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| D9 | 1 | a | 0.544939 | 0.469268 | 35 | 256380 |
| D9 | 1 | b | 0.529635 | 0.482909 | 35 | 256380 |
| D9 | 1 | c | 0.544606 | 0.477918 | 35 | 256307 |
| D9 | 1 | d | 0.505349 | 0.476421 | 35 | 256380 |
| D9 | 2 | a | 0.528624 | 0.478383 | 35 | 256380 |
| D9 | 2 | b | 0.524964 | 0.461583 | 35 | 256380 |
| D9 | 2 | c | 0.525630 | 0.479215 | 35 | 256380 |
| D9 | 2 | d | 0.488370 | 0.473393 | 35 | 256380 |
| Contained | 1 | a | 0.616439 | 0.567650 | 48 | 351579 |
| Contained | 1 | b | 0.349404 | 0.344775 | 34 | 248984 |
| Contained | 1 | c | 0.354386 | 0.353743 | 24 | 175754 |
| Contained | 1 | d | 0.367671 | 0.354740 | 34 | 248984 |
| Contained | 2 | a | 0.606180 | 0.599609 | 48 | 351579 |
| Contained | 2 | b | 0.348097 | 0.360762 | 34 | 248984 |
| Contained | 2 | c | 0.367694 | 0.360098 | 24 | 175754 |
| Contained | 2 | d | 0.373008 | 0.355115 | 34 | 248984 |

These are shared source-cache packet charges, payload plus 64 bytes per packet,
not RSS or client output queues. Peaks cover each view's lifetime, including
warm-up. Each tile has exactly one group, so no sum-of-group peak ambiguity
applies. Current-1 has private readers rather than this source cache; no fake
zero-byte comparison is assigned to it. All measured refresh, deferral, reset,
restore and recovery-failure counters stay zero.

Store batch/barrier traces and per-PID limiter STOP/CONT brackets are complete
in every row, with no dropped events. The largest store barrier is 23.2354 ms
and the largest store STOP upper bound is 12.4413 ms in these fresh cells.
Those successful-cell bounds do not explain away the earlier availability
event. Its cause remains unresolved, and per-renewal instrumentation is not
claimed. M2 uses the measured frozen bytes and stops on any lease expiry.

## Contract and current path

D3 allows foreign terrain and block updates, not foreign entities. E22 will
keep foreign players, mobs, dropped items and their metadata hidden. The real
player lane keeps its existing entity path. No halo behavior is added.

E18 opens a separate synthetic session for each nearby player and tile.
`meridian-proxy/src/terrain.rs` owns admission, recentering, refresh and reader
retirement. Its current scheduler is E20's patch 07 plus the stop-race fix.
`meridian-handoff/src/terrain.rs` owns column composition and crossing
readiness. `session/reactive.rs` answers each reader's flow control and routes
its packets to just one session. `composed_arrival_ready` requires the old
source's passive reader to hold every still-visible source column before the
real source connection can disappear.

Horizon currently supports one position and a radius from 2 through 32 per
reader. `Hub::update_view` queues that single tracking area, and
`Game::lan_update_streaming` holds the matching loading area. Radius changes
alone cannot represent disconnected groups along a 2,048-block wall without
loading the gaps or opening more readers as the groups spread out.

## Chosen design

Use one shared reader per configured tile and proxy. Keep one client
subscription per nearby player, including an own-tile subscription near a
wall. Each subscription contributes an area centred on that player's chunk,
with the existing one-ring terrain lookahead. The view stores the union, not
one copy of every contribution. Equal areas share reference counts. Overlap
shares column interests. Tile ownership filters every column.

The implementation permits up to four registry-compatible groups per tile.
Each group has its own reader, digest and cache. This preserves clients whose
configuration negotiation differs from the first client's. Identical native
D9 clients use one group. A fifth simultaneous incompatible group is refused
before it can mix packet identities. Retired groups release their slots.

The existing admission distances stay unchanged. Subscribe within the client
radius plus two chunks of a tile. Keep the subscription until radius plus
four chunks. The own-tile subscription remains while any foreign tile is in
that hysteresis band. Grow requested coverage before exposing a subscription
as ready; remove coverage only after the final subscriber releases it.
Session teardown removes only that subscription. It cannot close a reader
still used by another player.

The view has a fresh synthetic UUID, an incarnation number, desired coverage,
acknowledged coverage, received columns, bounded cached packets and subscriber
tokens. A token includes a session generation. Work queued for an old token
or view incarnation cannot deliver into a replacement subscription. The last
subscriber retires the reader intentionally and drops its cache. Registry
compatibility is checked for every subscriber, not only the first one.

## Horizon patch 11

Add an optional bounded area list to `player-observe`. Preserve both existing
position forms. The request supplies an anchor position, an interest revision
and deduplicated chunk-centred areas. An empty list represents no demand.
Validate coordinates, radii, area count and expanded column count before
changing any state. Reject the complete request if it exceeds the bound.
The reply echoes the accepted interest revision. An acknowledgement means
the request was installed, not that its chunks have arrived.

For this reader only, replace single-centre tracking with the requested union.
Queue additions once; forget only columns outside the union. Use the same
areas for actual chunk loading, with the existing lighting-neighbour margin.
Do not turn disjoint areas into one large square. Ordinary players and legacy
observers retain their current paths. No cut, lease, import or activation
behavior changes. The observer remains held, unplaced and unimportable.

Add a bounded refresh operation for selected still-requested columns. It
requeues full snapshots on the same ordered game stream without sending a
Forget first. This supplies a fresh cache base after update-journal saturation.
It does not reset the whole shared reader or any other client's view.

Keep the initial limits explicit and identical in both programs: at most 256
distinct areas and 4,096 union columns per view. Validate the expanded loading
coverage too. Control requests must stay within the existing line-size limit;
the implementation must measure serialized size and refuse oversize input.
No silent clipping of requested coverage is allowed.

## Meridian patch 08

The executable owns the shared-view registry and its lifecycle. A private
shared-view module in the handoff crate owns ordered cached terrain delivery.
The existing per-session terrain module keeps ownership, client loaded sets,
ghost state and the crossing guard. No new crate or independent runtime.
Existing bounded control workers do login and control IO. The existing IO
pool carries each reader. No control socket call runs on an IO driver.

The proxy reconciler changes from private reader ownership to interest
subscriptions. It sends a union update only when coverage changes, plus the
existing five-second held-reader refresh. Each view has at most one control
operation in flight; newer interests replace queued obsolete interests.
Worker saturation retries without discarding the current live view. Known
admission refusal retains the one-second backoff.

These edits overlap E20's `meridian-proxy/src/terrain.rs` and may overlap
`meridian-handoff/src/session/reactive.rs`. Shared views should apply after the
reviewer-selected E20 delta. They do not need to change `meridian-net`'s socket
scheduling policy. The proposed stack is both patch-07 files, the selected
E20 follow-up if any, then `meridian-08-shared-views.patch`. Horizon patch 11
applies after `horizon-10-warm-tick.patch` and its proved prerequisites.

## Cache, client ordering and updates

Each requested column keeps one immutable full chunk packet plus ordered
persistent updates since that base. Split multi-column biome packets before
caching and routing. Cache block, section-block, light, block-entity and biome
state. Forward sounds and block or level events live only; do not replay an
old effect to a newly subscribing client. Unknown packet classes do not enter
the terrain stream. Foreign entity packets never enter either cache or fanout.

A subscriber receives a snapshot and subsequent updates as one ordered
sequence. Joining the live stream and selecting the cache revision happen
under the same view ordering boundary. This prevents both a missing update
between snapshot and subscribe, and a newer update preceding an older base.
Do not hold the global registry lock while taking session locks or writing.
Use weak session membership so the shared view cannot keep a closed player
alive. Never take a view lock from inside a session-lock critical section.

Each client keeps its own centre, radius, delivered columns and signatures.
The shared observer's centre, radius, batch boundaries, teleport, login,
keepalive and acknowledgements are not client packets. Answer those once on
the reader. Deliver chunks through the existing owner-filtered composition
kernel, using a client-local load sequence. A client receives additions before
out-of-view Forget packets. One client moving away cannot unload another's
columns. A client already holding an unchanged column is not sent it again
when another client joins or an area changes.

Persistent updates fan out only to clients holding that column, in stream
order. Own-tile subscriptions record cache readiness without duplicating the
active lane's visible traffic. The existing crossing check uses the shared
view's received-column proof for the old source, tied to the current
subscription and incarnation. Having sent an interest request is not enough.
No cut, freeze, CAS, activation or source-release ordering changes.

A client respawn clears and resubscribes only its own composition. A teleport
changes only its interests. Neither operation resets a shared reader. A lost
reader invalidates readiness and fails affected live subscriptions closed;
it must not silently leave stale foreign terrain on screen. An intentional
last-subscriber retirement is not a connection failure.

## Memory and backpressure

Start with 64 MiB of full chunk bases and 16 MiB of persistent updates per
view, a 256 KiB update journal per column, and the coverage limits above.
Account retained packet bytes, not only map entries. Each client's existing
8 MiB output cap remains. Shared packet references do not remove that cap.

At a column journal limit, invalidate only its replayable snapshot, discard
the obsolete bounded journal and request one fresh full snapshot. Existing
clients continue receiving live updates without a chunk reload. A new client
waits for the fresh snapshot; it cannot use a stale base. Allow at most one
refresh in flight per column and bound the refresh queue by requested columns.
The stream-ordered replacement snapshot establishes the new replay base.

Reject a new subscription before changing installed coverage if its required
columns or replay admission cannot fit. Existing subscribers keep their
coverage. If an incoming legal packet cannot fit even after dropping unused
cache entries and invalidating replay state, fail affected subscribers closed
with a recorded resource error. Never claim readiness with missing columns.
The implementation must test the reservation and overflow paths explicitly.
These limits bound one view; total view count is bounded by configured tiles.

## Saving estimate from E21

The contained-cohort control, not the confounded terrain-off crossing run,
measured 650 real players and zero handovers in both arms. Reader-on used
65 readers per tile, 260 total. Reader-off used zero. The observed difference
was 0.16003 core in four tiles and 0.27197 in the proxy, 0.43200 total.
Production-server CPU was 2.78659 with readers and 2.35460 without them.

Four shared readers would reduce session count by 98.46 percent. If every
measured reader cost scaled only with session count, the illustrative saving
would be `0.43200 * 64 / 65 = 0.42535` core, leaving 2.36124 cores. That is
an optimistic illustration, not a forecast. The union still loads chunks,
encodes changes, fans packets out and retains per-client state. E21's tile
delta includes extra resident chunks and preparation, which sharing does not
remove. E20's manager changes also make part of the old proxy cost inapplicable.

Use 0.15 to 0.30 core total as a planning hypothesis for that contained
650-player control, about 5.4 to 10.8 percent of its reader-on total. It is
not a measured range or statistical interval. The useful hard result before
measurement is the session reduction; the CPU saving must be established by
a matched pair. Do not subtract either estimate from the D9 crossing row or
extrapolate it to 975 players. No new frontier follows from this design.

## Proof and measurement order

1. Unit and protocol tests cover overlapping and disconnected areas,
   independent unsubscribe and unload, negative-coordinate ownership,
   update-before-join ordering, block-entity and biome replay, bounded journal
   refresh, stale incarnation rejection, admission refusal, lost readers,
   own-source readiness, teleport and respawn isolation. Verify held/read-only
   behavior and foreign entity exclusion with a positive owner control.
2. Prove clean numbered-patch application and source byte equality. Run scoped
   formatting, strict release Clippy, release unit and TCP tests, then build
   one release profile with `CARGO_INCREMENTAL=0`. Freeze all executable and
   launcher identities under `.local/bin/meridian-e22/<name>/`.
3. Request a fresh quiet window as E22. Wait for `.local/quiet-ack-E20` and
   start load at most four. Each window is under 30 minutes, released after
   its cell. If E20 requests the host, finish the current cell and acknowledge
   via `.local/quiet-ack-E22`. No build overlaps a measurement window.
4. Compare the current frozen stack and shared-view stack at 650 on the same
   D9 crossing fixture with readers enabled, 30 seconds warm-up and 60 seconds
   measurement. Keep engines identical within a pair, using patch 11 in both
   arms with its legacy observer path in the control. Report proxy, every tile,
   store, sidecars, total production CPU, reader counts, union chunks, raw
   ticks, bots and outside-process noise. Add the contained-cohort pair to
   separate reader savings from changed handover work if the reviewer agrees.
5. If clearly cheaper, run every E18 final M2 command on the frozen bundle.
   Physics uses `scripts/e20-build10-check-v2`, never a relaxed threshold or
   an overwritten reference. Retain failed and unresolved windows. Results
   count only after independent reviewer reproduction.

All result rows redact outside process names to `outside`. Screenshots and
environment traces stay under `.local/evidence/E22/`. Worlds and logs stay
under `.local/runs/` until fixture cleanup. Run plain `scripts/clean-builds`
at task end, never its all-target variant.

## Frozen paths and reproduction commands

Frozen engine:
`.local/bin/meridian-e22/observer-2/horizon-client`

SHA-256:
`21805ed6621cf138f0e3d509a91d233bee8f8fe182f625e3fcaa1bad88f87ea6`

Patch: `patches/horizon-11-shared-observer.patch`, SHA-256
`3511c8b2f4d8e0ef4a33d42867c54865259ee39d92f0b5f37fafd07c6fa10873`.

Source proof: `data/E22/stack-proof.json`, SHA-256
`ce4ea6a1744527e11715de76368cccc8404de74f2e90b292858920f019372c8f`.
The build record is `data/E22/observer-2-build.json`.

The frozen bundle keeps reactor-1 Meridian and all support executable bytes
unchanged. Its leased snapshot validator and all executable hash checks pass.
Existing E18, E20 and E21 snapshots remain unchanged.

From the repository root, in the already-proved owned Horizon checkout:

```sh
python3 -B scripts/e22-stack
python3 -B scripts/e22-build --name reproduce-observer
```

After the build finishes, run compatibility checks with fresh result names.
Each command requires a fresh E20 acknowledgement and starts only at load
at most four. Each window releases after fixture cleanup.

```sh
python3 -B scripts/e22-window --name reproduce-observer-terrain -- python3 -B scripts/e22-check terrain --bins .local/bin/meridian-e22/observer-2 --crossings 100 --name reproduce-observer-terrain
python3 -B scripts/e22-window --name reproduce-observer-borders -- python3 -B scripts/e22-check borders --bins .local/bin/meridian-e22/observer-2 --name reproduce-observer-borders
scripts/clean-builds
```

## Implemented Horizon control shape

The sparse request uses chunk coordinates for area centres, and block
coordinates for the observer's otherwise independent anchor position.

```text
player-observe UUID {"position":[0,-60,0],"revision":1,"areas":[{"center":[-100,0],"radius":2},{"center":[100,0],"radius":2}]}
player-observe-refresh UUID {"revision":1,"columns":[[-100,0],[100,0]]}
```

The first response echoes revision 1 after installing the validated union.
The refresh response echoes the revision and counts newly queued replacements.
Repeated refresh requests coalesce while a replacement is pending. Refresh
requires a joined held reader and columns already delivered in its current
union. It never removes sent membership while waiting for a full replacement,
so block and light changes keep flowing on the same ordered connection.

A lower interest revision, or an equal revision with different areas, is
refused. Equal revisions with the same deduplicated areas refresh the held
timeout. An empty area list shrinks the union to zero. A sparse reader cannot
silently switch back to the radius mode through a legacy refresh request.
Legacy readers never acquire sparse state unless explicitly requested.

Both request payloads have a 64 KiB parsing limit. Refresh accepts at most
256 column entries. Sparse admission bounds the union's actual loading square
margins as well as transmitted columns to 4,096. This means some legal legacy
radii, such as 32, exceed the sparse coverage budget and are refused only in
the new mode. The existing radius-32 mode is unchanged.

Sparse readers do not add a separate loading ticket at their anchor. Their
explicit areas alone drive loading. They receive interior light updates too,
because a column inside the union may be on an individual client's view edge.
The legacy light-edge filter remains unchanged.

## Implemented Meridian ownership

The executable's session registry owns weak shared-reader slots. A per-player
subscription keeps its slot alive; the IO callback holds only weak player
membership. No player's exit can close a reader still held by another player.
The handoff crate owns cache replay, ordered fanout and client-local delivery
sets. The existing terrain owner still filters packets and proves old-source
coverage before crossing. Blocking transport keeps the original private-reader
path. `E22_SHARED_OFF=1` selects that path explicitly for diagnostics.

Coverage changes update a local version counter. Unchanged membership does not
rebuild the union. Cache refresh requests coalesce per column until a full base
arrives. Saturated journals release their retained vector capacity. Alongside
packet-byte bounds, each group allows at most 4,096 subscribers and 262,144
subscriber-column interests. Requested loading coverage stays at 4,096 columns.

The first shared-view release tests passed all 104 handoff cases and the
unchanged config, ghost, network, proxy, signal and sidecar suites. The fixed
configuration-group bound adds one proxy test. Two later strict-gate attempts
rejected test-only lint issues and a complex registry type; these were corrected
without suppressions. All failed gate records remain under `data/E22/`.

The final patch-08 release run, `shared-gates-5`, passes 105 handoff and 566
proxy tests. The added reader-state test proves entity and advancement
histories never accumulate in the terrain reader's flow-control target.
Config 202, ghost 33, network 107, signal 2 and sidecar 99 pass too. The same
one internet test is ignored and the known listener-burst test is skipped by
its exact unchanged name. Formatting, strict release Clippy and the release
binary build pass. A frozen 14-crossing native terrain smoke passes as well.

## Fixed CPU comparison in progress

This first comparison belongs to superseded `shared-1`. It does not qualify
the corrected candidate. See the cache-lifecycle section below.

The precommitted ledger is `data/E22/shared1-fixed-pairs.json`. It fixes two
current/shared pairs under D9's four-server-core and one-bot-core caps, then
two uncapped contained-cohort pairs. D9 uses 30 seconds warm-up and 60 seconds
measurement. The contained control retains E21's 30/30 durations and placement.
Each cell creates and releases an E22 quiet request. The reviewer confirmed
E20 had finished, so each raw row records the explicit acknowledgement waiver.
Start load remains at most four and outside-process validity is unchanged.

The first D9 pair passes both arms with all 650 players online, no bot lateness,
no dropped raw ticks and valid outside noise. Readers fall from 65 to one per
tile. Current/shared proxy CPU is 1.009959/0.911391 core. Four-tile CPU is
2.129696/1.905953 cores, and total production-server CPU is 3.150799/2.828156.
The observed total saving is 0.322644 core.

The repeat passes both arms too. Current/shared proxy CPU is
0.910442/0.864536 core; four-tile CPU is 2.112945/1.990578 and total server
CPU is 3.034198/2.866092. Total saving is 0.168105 core. Both builder repeats
therefore exceed the reviewer's 0.10-core threshold. This is repeated builder
evidence, not independent acceptance. The contained controls remain pending.

All four cells retain chunk counts 368/367/367/367. Activation counts in the
60-second windows are 590/587 for current/shared in repeat one and 592/587 in
repeat two. Crossings were not removed, unlike E21's rejected terrain-off
ablation. The contained zero-handover comparison will separate that remaining
small throughput difference from reader mechanism cost.

## Cache-lifecycle regression and follow-up

Local interest and source-sent coverage have different lifetimes. A client can
leave and re-enter an area before Horizon's next chunk-view tick. If the proxy
evicts immediately on local interest removal, Horizon can still consider the
column sent and never send a replacement. That leaves re-entering subscribers
without a cached base. Steady D9 coverage does not expose this ordering.

The added wire regression fails on `shared-1` production code. It keeps the
source stream alive, moves interest away, sends a persistent update without a
source Forget, then returns. The retained test-only patch, before-source proof
and failed command result are `data/E22/cache-race-regression.patch`,
`cache-race-before-source.json` and `cache-race-before.json`.

`meridian-08-cache-source-retention.patch` follows the immutable first patch 08.
It retains and updates source-sent snapshots until an ordered source Forget or
reader teardown, even while locally unused. Client-local Forget packets remain
independent. A source-sent column cap of 4,096 supplements the existing byte
bounds. Refreshes for locally unused columns wait; an already-sent precondition
refusal during control/game-socket reordering defers rather than closing a
client. New tests cover this gap, a replacement subscriber on a live stream,
and the source-cache entry cap.

The first fixed plan stopped after six completed cells. Its seventh row was
interrupted while waiting for start load at most four, before any fixture
started; the eighth row never started. All completed and interrupted evidence
remains. `data/E22/shared1-fixed-abort.json` records this withdrawal. No frozen
binary or previously frozen patch changed. A fresh complete matched plan is
required for the follow-up before M2.

## Final recovery barrier and telemetry

The final stack adds `meridian-08-refresh-interest-barrier.patch` after the
cache-retention follow-up. Refresh retries are paced per view at 100 ms. After
one second the controller requests empty interest, waits for the source's
ordered Forget packets, then restores the latest union. It does not use
`player-observe-reset`, whose sent-set clear emits no Forgets. Crossing source
proofs stay invalid during clearing. A five-second clear/restore-data deadline
fails the group closed; recovery control calls have two-second IO timeouts.
These are cooperative software deadlines, not a hard OS scheduling guarantee.

The permanent-unsent test keeps refusing the refresh precondition and delivers
the waiting protocol client's column at 1.1 seconds of injected monotonic time,
after the ordered barrier and restoration. Other tests withhold Forgets or
post-restore data and verify fail-closed EOF at the deadline. Release formatting,
strict Clippy and all scoped tests pass in `shared3-gates-1`: handoff 112,
proxy 566 and the unchanged suite counts and exclusions. Native 14-crossing
terrain and the real console telemetry probe pass on the frozen bundle.

`e22-views` reports per-tile/group current and peak source-cache columns and
charged bytes, plus request, deferral, reset, restore and failure counters.
Charges are packet payload plus 64 bytes per retained packet, not process RSS.
Peaks include warm-up. The fresh cell adapter captures them before and after
the CPU span. Per-tile aggregation sums group high-water marks; it is exact
with the one group used by D9 and the contained control.

## Observed lease availability event and qualification hold

The fresh `shared3-fixed` plan failed in its first current-control cell before
the new proxy ran. All four tiles reported local lease expiry and the bots
disconnected. The start load was 1.017. Cold and warm outside-noise checks
passed, but steady and whole-cell sampling did not complete, so the row is
not a CPU result. Server limiter STOP wall time totals 0.544 seconds across
71 cycles. Cause is unresolved; no lease rule, budget or binary was changed.
Raw failure and pre-cleanup evidence remain beside
`data/E22/lease-availability-event-1.json`.

The failed sampler's maximum interval was 1.045 seconds. Sampling is roughly
once a second, and a passing earlier control had a 1.050-second maximum.
That number alone does not establish an extra one-second host stall. The
simultaneous expiry remains an observed availability event either way.

An unchanged-configuration retry control completed and passed its noise and
diagnostic gates. Its shared arm was interrupted during startup after the
reviewer required per-renewal send/reply/grant telemetry, store batch/fsync
timings around gaps over 200 ms, and store-PID STOP timestamps in every later
cell. The frozen Horizon renewal loop does not expose those send/reply records.
Existing store tracing and harness STOP logging can supply part of the request,
but true renewal timestamps require an instrumentation overlay. Qualification
was held while the instrumentation boundary was clarified. Both plans and the retry
control remain; none is relabelled as final qualification. No M2 campaign ran.

The reviewer then narrowed acceptance telemetry to what the harness can obtain
without changing executables. `shared3-traced-fixed-pairs.json` runs a fresh
eight-cell fixed plan on the original frozen `current-1` and `shared-3` bytes.
Existing `GOLIATH_STORE_TRACE` is enabled identically in both arms; its work
remains in measured store CPU. The harness retains exact per-PID limiter
STOP/CONT call brackets for the store and each tile in memory, including final
limiter resumes. Signal serialization and store-trace harvest happen after the
CPU span. Caps, lease duration, margin and renewal rules are unchanged.

Complete batch/barrier CSVs and signal records are retained, with neighboring
batch context for spacing or durations over 200 ms. Batch spacing can be idle
time and is not labelled a stall by itself. Store-relative trace origin is
bracketed between process spawn and observed LISTEN, not given a fabricated
exact wall-clock origin. A bounded event buffer reports overflow explicitly;
incomplete capture cannot qualify a row. The six harness tests include a real
probe of the unchanged frozen store's existing trace flag.

Per-renewal tile send/reply instrumentation is deliberately excluded from
acceptance data. If any later cell latches lease expiry, the fixed plan stops
and reports the retained evidence. A separately named diagnostic investigation
then belongs to the reviewer. The earlier availability event stays in this
report even if the new fixed plan passes.

## M2 campaign ledger so far

`shared3-m2` completed all 1,000 actual crossings with zero disconnects, then
failed at restart verification because the availability wrapper did not forward
the fixture's `cut=` keyword. That is a harness contract failure, not a passed
restart or a production finding. The raw proof, outcome and command log remain
failed. Availability capture was complete and observed no lease expiry.

The wrapper now forwards positional and keyword arguments unchanged. An exact
contract test covers the `cut=False` call and return value. The native store
probe also caught a partial-CSV-line wait race; its wait condition now requires
a complete parsed row. The parser itself already refused incomplete lines.
All seven harness tests pass. The exact helper used by the CPU comparison is
retained as `data/E22/harnesses/availability-cpu-v1.py`, matching the recorded
raw helper hash. These changes do not alter CPU-span behavior or executable
bytes. The full command set is rerunning with fresh prefix `shared3-m2-r2`;
the earlier 1,000 crossings are not reused as a successful proof.

The rerun `shared3-m2-r2` passes the 1,000-crossing proof, all four restarts,
inventory equality, sole generation-1,000 owner, 100 terrain crossings, items,
all 16 borders, live state, teleport and lifecycle/fast approaches. Input-gap
P50/P95/P99 is 103.694/130.680/143.866 ms, with zero maximum correction and
1,000 visible CAS generations. Availability and noise captures pass throughout.

Its particle-off physics collection then fails after 42 actual crossings,
before the fly mode. The real-key flight activation guard fails. No thresholds
or key timings changed; particle-on and checker v2 did not run in that campaign.

## Native client-cache presence regression

The failed trace reconstructs exactly the renderer's final 80 columns instead
of the expected 81. The only missing column is `(4,4)`, directly under the
player at the setup teleport `[64,-60,64]`. It arrived initially, then a centre
move to `[-5,4]` evicted it from client storage. No later full chunk packet for
that column was forwarded. Initial server ability flags were 13, allowing
flight, and Space was present in captured client frames; no client ability
packet was emitted. The missing terrain is a concrete production defect, not
evidence that the flight guard should be weakened.

The composer cached old full-packet signatures without requiring that its
client ghost still held the column. Shared replay also hid owned columns even
when missing. The new unit fails on `shared-3`. Its test-only patch, before
proof and failed result remain under `data/E22/client-cache-presence-*`.
The packet reconstruction is `client-cache-presence-native-evidence.json`.

`meridian-08-client-cache-presence.patch` follows immutable shared-3. A duplicate
full chunk is suppressed only while the ghost actually has it. Replay considers
missing ghost columns, not merely a subscription's delivered set. An explicit
delivery mode permits authoritative owned-cache restoration with its ordered
persistent updates, while still hiding redundant passive owned traffic.

Release formatting, strict Clippy and all suites pass: handoff 114, proxy 566,
all other counts and exclusions unchanged. The frozen `shared-4` native probe
crosses a-to-b, evicts through an interior reset, returns b-to-a and resets to
`[64,-60,64]`. Column `(4,4)` is present and the exact unchanged real-key sequence
emits flying flag 2. Availability is complete with no lease expiry. Shared-3
performance or M2 evidence is not reused as qualification for shared-4.

## Frozen shared-3 at its earlier checkpoint

Candidate directory: `.local/bin/meridian-e22/shared-3/`.

Proxy SHA-256:
`944ca279d236b1064d296c4adb5bbea31f192d25c2c3ff99f3df5432b594e483`

Engine SHA-256, unchanged frozen patch 11:
`21805ed6621cf138f0e3d509a91d233bee8f8fe182f625e3fcaa1bad88f87ea6`

The matched control is `.local/bin/meridian-e22/current-1/`, with scheduler-2
proxy SHA-256
`8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad`.
Every other executable, launcher and lease-marker byte matches the candidate.

The exact Meridian stack after both patch-07 files is:

- `meridian-08-shared-views.patch`, SHA-256 `4e4814416a1b08e902ecb6aa4cad3e3b261bcd97654809d981865c7a66884a86`.
- `meridian-08-cache-source-retention.patch`, SHA-256 `45ef99df12c9c9e7e8c6f35091a12277671d9246266ef01161f41d21355a6d67`.
- `meridian-08-refresh-interest-barrier.patch`, SHA-256 `cee918a1ce05e593dae8672e928efe26c61d61c0f93f490f5353a90879a60b34`.

Final source proof SHA-256:
`3726186e495af28095b09e4bede4634c0f20f36600b8e09273424f18029a0a87`.
The candidate retains its own `proxy-source.json` and executable contracts.
Static/release reproduction from the already-proved checkout uses fresh names:

```sh
python3 -B scripts/e22-stack
python3 -B scripts/e22-proxy-artifacts patch --after .local/bin/meridian-e22/shared-2 --patch-name refresh-interest-barrier
python3 -B scripts/e22-proxy-artifacts gates --name reproduce-shared3
```

Do not start another CPU plan without the newly required harness telemetry.
The existing fixed plan and M2 entry points retain exact commands in their
ledgers; M2 additionally refuses a comparison that did not complete with two
valid D9 savings of at least 0.10 core on the same shared-view bytes.

## Verification ledger

The first release observer run passed nine tests and failed the refresh-order
fixture. Its first chunk batch was never acknowledged, so the server correctly
withheld the replacement snapshot. The fixture now acknowledges that batch
after first proving that the snapshot stays queued without an acknowledgement.
The exact block-update, batch-start, full-chunk, batch-end, block-update
assertion is unchanged. The failed log and build record remain in
`data/E22/observer-1-*`. No failed test result is called a pass.

The corrected release run passes all ten observer tests. They exercise actual
encoded packets through the hub, not only the area calculator. Strict release
Clippy passes for both `horizon-client-lib` and `horizon-client`. The release
executable build passed in 237.713 seconds. The numbered stack applies cleanly and exactly
matches 63 changed source files, with internal store dependencies checked
separately. That early freeze did not claim runtime verification; the later
legacy checks passed 100 terrain crossings and all 16 owned borders.

## Final hold handoff, cleanup and exact reproduction

`data/E22/shared4-handoff.json` binds the final hold decision to the frozen
executables, source proof, complete comparison, release gates, presence probe
and availability-event evidence by SHA-256. Readback verifies all 253 Meridian
source identities, all 63 Horizon changed-source identities and every required
snapshot executable/launcher hash. Whitespace and public-result home-path scans
are clear. Outside-process names remain `outside`; screenshots and environment
traces stay local. No owned fixture, build or quiet marker remains active.

Plain `scripts/clean-builds`, never its all-target variant, completed and freed
0 MB. Targets are 38 MB at the root, 2,018 MB for Horizon and 683 MB for
Meridian, all under their caps; the shared project is 8,427 MB. The user's
existing dirty baseline and older source checkout are preserved. No commit,
push, upstream submission or milestone checkbox change was made.

Shared-4 is unqualified/hold, not a production candidate. No full shared-4 M2,
new frontier, stock-client acceptance, wide-view soak or cross-host acceptance
is claimed. The focused presence/flight probe and release gates pass, but the
two-repeat cost rule does not. The reviewer requested no further optimization,
resampling or averaging. Frozen metadata retains its original freeze-time
pending labels; this report is the final status, not an overwritten manifest.

Frozen directory: `.local/bin/meridian-e22/shared-4/`.

Proxy: `meridian`, SHA-256
`9b5dfacb43e1881f5725a8341fc59d655af924e81e2829ea49b951a14f2df275`.

Engine: `horizon-client`, SHA-256
`21805ed6621cf138f0e3d509a91d233bee8f8fe182f625e3fcaa1bad88f87ea6`.

Source proof: `proxy-source.json`, SHA-256
`f4ed5e7adb8848895da9ceaec3c24c02fcfcaf32676204ee78618ff7c13c62e4`.

The control remains `.local/bin/meridian-e22/current-1/`, proxy SHA-256
`8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad`.
Engine, store, sidecar, bots, packet client, launcher and lease marker match
shared-4 exactly. Patch 11 stays frozen, SHA-256
`3511c8b2f4d8e0ef4a33d42867c54865259ee39d92f0b5f37fafd07c6fa10873`.

After patch 06 and both patch-07 files, apply the immutable Meridian deltas in
this order:

| Patch | SHA-256 |
| --- | --- |
| `meridian-08-shared-views.patch` | `4e4814416a1b08e902ecb6aa4cad3e3b261bcd97654809d981865c7a66884a86` |
| `meridian-08-cache-source-retention.patch` | `45ef99df12c9c9e7e8c6f35091a12277671d9246266ef01161f41d21355a6d67` |
| `meridian-08-refresh-interest-barrier.patch` | `cee918a1ce05e593dae8672e928efe26c61d61c0f93f490f5353a90879a60b34` |
| `meridian-08-client-cache-presence.patch` | `69abe7d1d7b01fecc0640ce41c7fdeeac372fa8b32f6406ac49c575f0256cdd2` |

All commands below run from the repository root and require fresh result names.
They are reproduction instructions for the reviewer, not another run by E22.
The full M2 entry point deliberately refuses this comparison because its cost
criterion failed.

```sh
python3 -B scripts/e22-stack
python3 -B scripts/e22-proxy-artifacts patch --after .local/bin/meridian-e22/shared-3 --patch-name client-cache-presence
python3 -B scripts/e22-proxy-artifacts gates --name review-shared4-gates
python3 -B scripts/e22-pairs --name review-shared4-fixed --current .local/bin/meridian-e22/current-1 --shared .local/bin/meridian-e22/shared-4 --no-other-agents
python3 -B scripts/e22-window --no-other-agents --name review-shared4-presence -- python3 -B scripts/e22-physics --snapshot .local/bin/meridian-e22/shared-4 --variant e12 --presence-probe --name review-shared4-presence
scripts/clean-builds
```

## Stored raw data

Raw result files larger than 1 MB are stored gzip-compressed as `*.json.gz`. Compression is lossless, and every SHA-256 recorded in this report refers to the decompressed bytes. To rerun a summary script on one of them, decompress it first with `gzip -dk <file>.json.gz`.
