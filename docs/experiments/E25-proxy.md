# E25. Proxy memory and latency

Status: final proxy is rebuilt reactor-1 80f, unchanged source. Its full-soak
RSS growth passed at 1.95% and the reviewer reproduced 2.59%. Frozen 29f
failed twice, at 16.45% and 23.02%. The build/runtime effect is unexplained.
M4 latency is not met under D9: final strict via P95 values are 11.159291
and 5.720209 ms. Final uncapped P95 is 4.709958 ms, explicitly not D9 or M4.
All E25 loads are finished. Scheduler-2 and mimalloc are rejected. Zone and
writev are unselected. The aborted zone entry has partial samples, no
completed measurement or soak. No capacity-frontier change is claimed.
The reviewer must reproduce a result before it counts. Nothing goes upstream.
No commit, push or milestone checkbox change is part of E25.

## Memory evidence

E24's dedicated proxy-memory handoff is the source for this investigation.
Its unwrapped frozen control recorded RSS rising 45.594 MiB while native
malloc residency rose about 45.6 MiB. Live allocated malloc bytes fell
19.5 MiB. Active threads stayed between 1,326 and 1,328. Stack mappings
stayed between 1,326 and 1,329, with stack residency 32.2 then 32.1 MiB.
This supports allocator residency and free space for that bounded interval.
It does not fully attribute the original growth. That remains open even if
a new allocator passes the memory line.

The counter-instrumented 650-player run held only 17.57 to 18.05 TPS.
Its live Rust heap fell 21,850,483 bytes while RSS rose 34.203 MiB.
Its approximately 2.53 million allocations/s and 1.88 GiB/s of requested
bytes suggest churn, but are perturbed diagnostics, not 20-TPS attribution.
The later working diagnostic crate is not the source of that binary.

## Original memory plan, superseded

First reproduce reactor-1 from pinned Meridian `902dcf1`, with the exact
seven-patch frozen stack and original internal store/model source.
Hash every source input and patch before compiling. Compare the rebuilt
proxy hash with the frozen control. If it differs, record the exact input,
toolchain and build-setting differences before any long pair.
Preserve the existing checkout and Git index byte for byte around each build.
Existing E22 work is not an E25 baseline and must not enter either build.

The first candidate selects the global Rust allocator only. It uses
`mimalloc = "=0.1.52"`, MIT, with `default-features = false` and no optional
features. That crate declares an empty default feature list. Pin the resolved
native wrapper and retain both registry checksums and licence identities.
Pin `libmimalloc-sys = "=0.1.49"`, also MIT, with no default features.
Its bundled native implementation is mimalloc 3.3.2. Defaults are
`purge_delay = 1000` ms, `purge_decommits = 1`, `arena_purge_mult = 1`,
`arena_eager_commit = 2`, `allow_large_os_pages = 0` and
`page_reclaim_on_free = 0`. These come from the exact crate archive,
not documentation for a different native revision.
Use native defaults. Set no `MIMALLOC_*` or legacy `MI_*` tuning variable.
The Rust wrapper archive SHA-256 is
`2d4139bb28d14ad1facf21d5eb8825051b326e172d216b39f6d31df53cc97862`.
The native wrapper archive SHA-256 is
`6a45a52f43e1c16f667ccfe4dd8c85b7f7c204fd5e3bf46c5b0db9a5c3c0b8e9`.
Its native implementation and Rust wrapper both carry MIT notices.
No timer, new thread, routing change, IO policy or allocator profiler is added.
The global-allocator mechanism follows the
[crate documentation](https://docs.rs/mimalloc/0.1.52/mimalloc/).

The residency evidence motivates testing an allocator with a different
free-page policy. It does not prove that mimalloc will return enough memory.
If default mimalloc fails, test purge/reset settings as a separately named
variant. If that fails, propose periodic macOS zone pressure relief as the
third variant. Every variant compares with the same reactor-1 control.
Never silently retune a candidate after seeing its result.

Queue the ready work through `host-window`. E24 has priority whenever it
waits; other work follows waiting time. Run a matched pair at 650 players.
Each cell has the unchanged D9 layout, 256 pigs, continuous
wall/corner crossings, four server cores and one separate bot core.
Use 30 seconds warmup and at least 900 measured seconds per cell.
Use identical fixed tile/store/sidecar/bot binaries within the pair.
The proxy alone changes. Both cells are tap-free.
The rebuilt control and candidate use the same relocated dependencies,
compiler and settings. The frozen binary serves only as the provenance check.

Acquire `host-window quiet E25` for the pair. After control cleanup, release
and requeue if E24 is waiting. Otherwise keep the window for the candidate.
Release after candidate cleanup. Never hold a window while building or analysing.
Start load must be at most four. No outside process may average above
0.5 core. Keep limiter stops and all histogram tails. Reject incomplete
populations, missing endpoints, dropped tick samples or authority failures.
Report each tile's TPS, raw tick P99, players, readers and pigs.
Proxy RSS growth from the nearest minute-5 sample to the final endpoint
must be strictly below 10%. Also report the intermediate RSS trend.
Do not call a slower or lower-throughput workload a memory fix.

Compare proxy CPU, thread count, forwarding counts/rates, internal
clientbound/serverbound/reader histograms and generated unstamped writes.
Report exact differences. Do not invent a regression allowance after the run.
Record page faults if the existing native counters expose them.
The later 30-minute M4 soak remains a separate qualification.

## Memory result, not qualified

The rebuilt control completed 900.140 s. Its nearest minute-5 sample is
299.788 s. All 883 measured bot reports retain 650 online players, with no
reported failures or late rounds. No authority-stop, clock anomaly or cleanup
failure was recorded. Start load was 2.816; maximum outside process averages
were 0.0780 core measured and 0.0778 over the whole cell.

| Process | Minute 5 MiB | Minute 15 MiB | Growth |
| --- | ---: | ---: | ---: |
| Proxy | 548.422 | 549.516 | 0.1994% |
| Store | 65.219 | 137.313 | 110.54% |

Every tile and sidecar stayed below 10% growth in this row. Their exact
endpoints are retained in the summary. This is not a qualified memory pass,
because two tiles fail the unchanged raw tick P99 line.

| Tile | Raw ticks | Dropped | Raw P99 ms | Game-time TPS |
| --- | ---: | ---: | ---: | ---: |
| a | 18,012 | 0 | 57.665542 | 19.9982 |
| b | 18,013 | 0 | 52.475750 | 20.0002 |
| c | 18,013 | 0 | 45.917208 | 19.9991 |
| d | 18,013 | 0 | 48.433666 | 19.9993 |

The retained generic gate message says P99 over 50 ms or dropped samples.
The actual failure is P99 on a and b. No sample dropped. No tail was trimmed.
Proxy CPU was 1.260740 cores with 1,333 peak threads. Internal clientbound
and serverbound P95 were 1.230 and 0.850 ms. Their counts were 567,821,117
and 35,311,618. These are narrow internal clocks, not proxy-added wire latency.
The server limiter stopped its fixture for 107.113 wall seconds in the
measured interval. Bot stops totalled 1.202 seconds. None is subtracted.

The candidate's retained row is preflight-only. It started no fixture or
measurement. The old preflight checked load once after acquiring the window,
so residual load above four after control cleanup aborted it. The fixed
preflight now waits up to 180 seconds for load at most four before starting
a fixture. Timeout still starts no load. Synthetic tests cover both paths.
The same readiness guard applies to the short latency cells.

The reviewer stopped further memory load. There is no candidate rerun,
control resampling or allocator-tuning result. Mimalloc remains ready and
unqualified. The control's near-flat RSS does not reproduce the historical
soak growth, so a 15-minute pair would not discriminate the candidate here.

Control summary: `data/E25/e25-memory-fixed-r1-control-summary.json`, SHA-256
`155b5dc2738d6f96b34bd7bed08fc750c226d4bac1218af411c7e71b460f5e2e`.
Candidate preflight: `data/E25/e25-memory-fixed-r1-candidate-summary.json`,
SHA-256 `c4e7daaec2cbd0490a05a83dba9ca4cd69b722e55807d5be75072d3441cfbce3`.
Both summaries identify the stored raw files and decompressed-byte hashes.

## Read-only historical RSS analysis

The retained E23 and E24 failed soaks both used frozen reactor-1 `29f1ff02`.
E24 used the fixed H13 tile/store bundle. Neither has a minute-30 endpoint.
The analysis reads the original RSS arrays, replay-before-release markers
and minute-spaced reader/player identities. It changes no original evidence.

| Run | Minute 5 MiB | Minute 15 MiB | Growth by 15 | First observed 10% crossing | Last retained point |
| --- | ---: | ---: | ---: | --- | --- |
| E23 | 592.969 | 648.266 | 9.3254% | 15:43.119 | 22:47.130, 691.953 MiB, +16.6930% |
| E24 fixed | 577.078 | 616.953 | 6.9098% | 18:43.318 | 27:51.088, 669.234 MiB, +15.9695% |

Growth does not start only after minute 15. It is already present before
minute 5 and continues through the retained windows. The 10% threshold is
crossed later. A 15-minute line can miss that continuing trend.
The largest adjacent positive changes after minute 5 are 0.703 MiB in E23
and 0.766 MiB in E24, each over about one second. They account for only
0.71% and 0.83% of the respective net increases. No large late step explains
the growth. The curves rise through many small changes.

| Observation | E23 | E24 fixed |
| --- | ---: | ---: |
| Crossing markers/minute, complete bins after minute 5 | 581 to 589 | 582 to 589 |
| Minute RSS change versus marker count, Pearson r | 0.253 | 0.069 |
| Reader identities in every minute audit | 260 | 260 |
| Sampled reader arrivals/departures between audits | 0 / 0 | 0 / 0 |
| Real identities in every minute audit | 650 | 650 |
| Sampled real arrivals/departures between audits | 0 / 0 | 0 / 0 |
| Online bots throughout retained reports | 650 | 650 |

Crossings remain steady while RSS rises. There is no sampled reader
replacement, real-player join/leave wave or reader-count increase at the
growth points. This does not rule out unobserved sub-minute lifecycle work.
The crossing marker is the native replay-before-source-release event, not
an independent completed-owner ledger. Its wall clock is aligned to elapsed
RSS time only for coarse minute bins. The weak descriptive correlations
do not identify an allocation owner or exclude steady allocation churn.

The historical soaks also seed 65 named cargo items and perform minute
ownership audits. E25's plain D9 control does neither. Its rebuilt proxy is
also hash-different from frozen reactor-1. These differences and uncontrolled
allocator residency limit any explanation of the new flat row. No cause is
assigned from this comparison. Full attribution of the original growth stays
open even if mimalloc later passes.

Any later allocator comparison needs the same soak work on both sides and
a window long enough to expose the retained trend. The final 30-minute soak
can carry that comparison once availability work permits it, with a matching
reactor-1 control. A lone short candidate pass would not prove the fix.
No new memory load or 30-minute retry is started by E25.

Analysis and input hashes: `data/E25/e25-rss-history-r1.json`, SHA-256
`16b7d7d665624ce64970d02faffc353e29f20eefb8920df9805140b907f0e491`.
It retains every RSS point, minute bin, sampled identity count and the largest
adjacent changes. Native proxy-log archive bytes were verified during the
streamed read. The original input files remain unchanged.

## Revised memory qualification plan, deferred

The next memory pair uses the soak harness itself, not plain D9.
Run rebuilt control first, then a replacement memory candidate. Default
mimalloc is no longer selected after its repeated latency regression.
Each run has the same 650
players, 65 named cargo items, minute owner audits, native sampling,
tick drains, endpoint recovery checks and a full 30-minute measurement.
Both use the same approved and frozen successor lease bundle.
No run starts before E24's renewal-cadence fix is approved and frozen.
An authority event voids the pair. There is no silent retry.

The observed historical slope is about 0.4 to 0.6 percentage point of the
minute-5 RSS per minute. It is not a late growth onset. The workload and
binary differences mean that slope is not a universal allocator rate.
The 30-minute soak can carry the comparison; it still needs a matching
control and the unchanged strict 10% line. Mimalloc is disqualified.
This plan supersedes the 15-minute qualification proposal, not its records.

### Replacement candidate order after the r2 soak

E24's r2 soak completed with no authority stop or owner violation, all 65
cargo items conserved and about 20 TPS on every tile. Its frozen reactor-1
proxy grew from 575.703125 to 670.406250 MiB between minute 5 and minute 30,
16.449993%. E24 deliberately did not judge the proxy. Against E25's line,
that endpoint growth is over 10%. It is not E25's matched rebuilt control.
The store receipt window filled only 31.889066 s before minute 5, which
remains explicitly fragile. That availability result is not a D9 capacity
pass. Its retained raw tick P99 values are a 62.802750, b 63.455625,
c 56.952834 and d 56.970500 ms, all with zero dropped ticks.
The source explicitly records `capacity_diagnostic_pass = false`.
Source result:
`data/E24/soak-fixed-r2-result.json`, SHA-256
`786ef62defccc20cd921c5e6093cba7929ae244f3cad47a78d26f9040f084ede`.

Candidate evidence is listed below. The approved soak run order follows
the table. A latency screen is not a memory qualification.

| Item | Variant | Evidence and limit |
| --- | --- | --- |
| 1 | Scheduler-2 alone | Removes roughly half the threads and saves about 0.09 proxy core in the r1 diagnostic. Fewer stacks and thread-local caches may reduce residency. RSS improvement is not established. Its 5.121833 ms diagnostic P95 is still over the line. |
| 2 | Scheduler-2 plus writev64 | The profile assigns 11.46% of Running CPU to allocations/copies and 39.88% to socket calls. The patch removes the descriptor Vec per vectored-write attempt, not all per-packet allocations. Payload/frame allocations and ready-set drain Vecs remain. |
| 3 | Zone16m60-r1 on the selected latency-safe scheduler base | Tests free-page return directly, as supported by E24's native malloc-residency evidence. Synchronous allocator work can contend with packet work. It needs its own gates and latency rows. |

Memory entry uses ordinary capped via rows on the all-gated r2 stamp
bundle. A candidate's overall P95 must be no worse than the matched
reactor-1 reference in that same session. This is not a strict-5-ms pass.
Report all tile P95 values, the worst tile beside overall, CPU, threads,
STOP brackets, forwarding counts, internal histograms and RSS. Final latency
qualification still needs two via repeats, both strictly below 5 ms.
No limiter, D9 rule, unmatched-infinity rule or tail treatment changes.

Only a candidate meeting the no-regression entry rule advances to a full
soak-harness comparison. No candidate currently proves the strict 5 ms line,
so that final line does not block memory investigation.
Build its reactor-1 control with the same compiler, dependency archive,
target selection and profile settings. Run that control first for a full
30 minutes, then scheduler-2 for a full 30 minutes. Next test zone pressure
relief on the leading base. Add writev once its latency row exists.
Every compared cell uses lease-fixed-r2
with the same 650 players, 65 cargo items, minute audits and sampling.
Memory cells use the frozen unstamped tile/bot fixture, not timestamp traces.
The exact proxy executable must later pass both memory and latency lines.
Any authority event voids the pair. There is no silent retry.

The zone proposal uses the existing `serve.rs::waker` accept-waker loop.
It adds no thread or wakeup. Make the first call after 60 s and then wait
60 s after each completed call, without catch-up bursts. Request
`malloc_zone_pressure_relief(NULL, 16 * 1024 * 1024)` once per period.
The 16 MiB goal exceeds the observed roughly 3.8 MiB/minute RSS rise, but
it is best effort, not a work or latency bound. NULL examines all zones;
a zero goal would request maximal relief, so zero is not selected silently.
These semantics match the local SDK header and Apple's
[public API reference](https://github.com/apple-oss-distributions/libmalloc/blob/main/_autodocs/api-reference/zone-creation-registration.md).
Record call count, released bytes and call durations. Each zone latency
cell must include scheduled relief calls. Keep continuous RSS and cycle
peaks, not just endpoints. Never force a call at the final RSS sample.
Any later interval, goal or base change gets a new named variant.

Prepared off-checkout: `patches/meridian-12-zone-pressure-r1.patch`, SHA-256
`daa84463a93a85e43d1af585f8f860122ebf05df0ae13f2f63ec40b96ca95303`.
It applies cleanly to the frozen scheduler-2 private index. It is not yet
built, gated or measured. It adds two deterministic scheduling/counter tests;
those tests do not invoke the native allocator API.

Static policy check found that this draft cannot pass Meridian's inherited
`unsafe_code = "forbid"` lint. No lint has been weakened and the draft has
not been compiled. A suitable small safe wrapper was not found in the
bounded API search. The reviewer approved one tiny, separately linted MIT
Darwin-interface crate with no registry dependencies, build script, threads
or runtime. This is a deliberate local exception at a real OS boundary.
The proxy and every existing crate retain the workspace `forbid` policy.
The new crate has `#![deny(unsafe_code)]` and exactly one scoped
`#[allow(unsafe_code)]` block around the fixed-NULL Darwin extern call.
Its safety comment cites `<malloc/malloc.h>` and covers every goal value.
The sole safe public API returns zero on other targets. Unit tests cover
the native call returning and the non-macOS no-op. Neither has run yet.

Prepared revision: `patches/meridian-12-zone-pressure-r2.patch`, SHA-256
`dd461819b5d1df76a4af718e8bdbfb17bd2842fbb9c3f717c30369a69cfc018b`.
Its private-index application and policy contracts pass. It remains an
unbuilt measured candidate, not a policy change to the production stack.
If the candidate fails the latency entry or RSS line, the wrapper is not
included in the selected stack. Earlier patch revisions remain as evidence.

Approved revision: `data/E25/e25-memory-next-plan-r2.json`, SHA-256
`cb269aecca7bc6f519351be9f60534aae0aeb17ff5afc78e7dd5739f56bc9156`.
The initial stricter entry proposal remains unchanged as revision r1.
The original growth is still not fully attributed. Passing a candidate
would not close that question.

## Which soak work reaches the proxy

This is read-only tracing of the executed harness and frozen reactor-1
source. Source identities come from the pinned seven-patch private index.

| Activity | Actual destination | Proxy work or retained state |
| --- | --- | --- |
| Seed 65 named items | Direct tile control `console give` | Indirect inventory packets pass through the proxy and update current per-session ghost inventory |
| Owner audit, twice 65 player reads/minute | Direct store `ReadUuid` | No proxy request; the returned bodies stay in the Python audit |
| Four `server-status` reads/minute | Direct tile control | No proxy request; snapshots stay in the Python audit |
| Tick drains | Direct tile `e11-samples` | No proxy request; accumulated ticks stay in Python |
| RSS/CPU/thread samples | Kernel `proc_pidinfo`, thread counters and `ps` | No application request or per-sample proxy entry |
| E24 authority/clock status polls | Direct tile `cluster-authority` | No proxy request; telemetry stays in tile/harness state |
| Internal path histogram reset/snapshot | Proxy stdin `e18-path`, start/end | Fixed histogram storage; snapshot allocations are temporary |
| Final recovery and saved-file audit | Tiles/store/files after stopping proxy | Cannot create new state in the stopped proxy |

`Soak.cluster` seeds through the original `Tile.ask`, not proxy chat or its
command hook. Its `sample` reads native task counters. `audit` brackets four
direct tile status replies with `Cluster.player` store reads. E24 adds
direct tile authority polls and native trace output. It does not send heap
commands to frozen reactor-1 in these soak rows.

Cargo is therefore a real proxy-facing workload difference. Frozen
`backend_packet` feeds play packets to `down.ghost.apply`. The ghost's
`Player` keeps the current inventory, carried stack and current open menu.
Full content replaces the inventory; slot changes replace an existing slot;
container close clears the current open menu. It is not a history of gives
or audit requests. The ordinary inventory menu has 46 slots in this code.
Named item components are real retained per-session values. Preparing and
executing crossings also clones/carries that state in target views and
player-export/import records. Repeated serialization, parsing and replacement
can change allocation churn even when the logical item count stays fixed.
That is a path to investigate, not a proved allocation owner.

The proxy's control pool retains connections, not a reply history: at most
eight idle connections per endpoint and 64 idle endpoint entries. Reply
JSON is a request result, not appended to the pool. The normal four-tile
soak has fixed live endpoints; harness audits do not use this proxy pool.
Path timing has four arrays of 10,001 atomic buckets, plus fixed counters.
It cannot grow one entry per sample. Allocator residency and retained
buffer capacity are still different from live request or inventory counts.

No unbounded proxy collection caused by the audit or sampling calls was
found in these traced paths. This is not a whole-heap proof and does not
close the original attribution. Inventory-bearing crossing churn is a
concrete difference to preserve in the later matched soak comparison.

Source pointers: `scripts/e23_observe.py:149`, `:189`, `:213`, `:165`;
`scripts/goliath-cluster:389`; `scripts/e20-frontier:136`;
frozen `meridian-handoff/src/session.rs:1204`,
`meridian-ghost/src/player.rs:92`, `meridian-sidecar/src/endpoint.rs:37`
and `meridian-handoff/src/path_timing.rs:11`.

## Latency plan

Memory qualification comes first. Instrumentation is not built yet.
This plan changes no pass line, limiter or validity rule.

Use timestamped bot movement sends and tile network-decode observations on
one host clock, `CLOCK_MONOTONIC_RAW` on macOS through the safe clock API.
Use absolute clock readings in both processes, not separately zeroed Instants.
Wall time is diagnostic only. The production path gets no extra socket,
Python tap or forwarding hop. The bot handshake must name its forced host
directly, removing the existing test-only handshake shim as well as both taps.

Preregister eight interior witnesses, two per tile, from seed 25 before
startup. Preserve their exact route anchors, speed, cadence and view distance.
The immutable registration is `data/E25/e25-latency-plan-r1.json`, SHA-256
`5d941aa29f128d1cc2958995d66556f628c7263f9974e5ac11d65637131ed70d`.
Its witness names are A3/A96, B54/B78, C10/C121 and D8/D65, each with
the common `E17Bot` prefix. The file records their full names and anchors.
All 650 players, 65 continuous crossers and 256 pigs remain in the via-proxy
D9 cell. Observe only witness movement, without allocation-stack profiling.
Timestamp each selected framed movement at its actual write syscall.
The bot hook is `Bot::flush` in `crates/horizon-bots/src/measured.rs`,
immediately before `self.socket.write(&self.output[self.sent..])`.
`Bot::send` has already encoded the packet and called `frame::encode`,
including negotiated compression. The measurement fixture uses offline login;
record its compression threshold and absence of encryption in trace metadata.
Precompute payload fingerprints and frame byte ranges before this hook.
Take the clock reading with no formatting, allocation or trace append between
it and `write`. After the syscall returns, assign that reading to movement
frames whose first byte it actually accepted. Zero-byte/WouldBlock attempts
are not duplicate sends. Partial frames retain their original timestamp.
A queued or partial witness frame never completed is reported and cannot
silently disappear from the denominator.

The tile hook is `lan::io_pool::Conn::read` in
`crates/horizon-client/src/lan/io_pool.rs`, in the
`Ok(Some((id, payload)))` arm immediately after `self.reader.buffered()`.
Capture the raw clock there before calling `self.session.packet`.
This is after protocol frame decode and before the network session can queue
`NetEvent::Packet` for the main thread. `PlayConn::packet` currently queues
that event in `lan/conn.rs`; a stamp there after tick dispatch is forbidden.
Pass the captured time to a bounded diagnostic hook that records the selected
session's full UUID and payload fingerprint on the connection reader.
No game-thread processing timestamp enters this clock.
These are the named bot-write and tile-frame-decode hunks of the planned
E25 latency patch. Record the final patch filename, SHA-256 and exact hunk
line anchors here after instrumentation exists, before running the cell.
The clock covers all intervening queues.
Keep trace buffering bounded. A dropped record invalidates the timing proof.

For the direct reference, replace those same eight interior sessions with
direct-to-tile sessions at exactly the same anchors and routes. The other
642 sessions still traverse the proxy, including all 65 crossers. The total
real-player load and gameplay stay equal per tile. Direct witnesses use the
same bot build and tracing code, with only the destination and initial
handshake routing changed. Readers stay enabled and their actual counts
must match. Report each tile's players, held sessions, readers and pigs.
Direct admission uses the normal authenticated initial-login path, as an
ordinary proxy login does. An expected transfer login would instead be held
and would change the workload. The direct native bot answers the existing
v1 forwarding request using only the fixture's throwaway secret file.
No registry, authentication, ownership or lease check is disabled.
The startup helper pins the already-locked `sha2` 0.10.9, MIT or Apache-2.0.
It runs before play, outside the measured send-to-decode interval. There is
no login relay or extra TCP hop.
Only the eight direct-reference witnesses receive the signing option.
Every via-proxy session, including witnesses, retains the normal proxy
connection without that option. Published raw data has one path row per
actual launched and placed player. It records no secret, signed login
payload or signature. The fixture's signing material stays local.

After profile verification, both paths use the same tile `PlayConn`.
Its fields do not contain a forwarded-versus-proxied dispatch mode.
Matched movements run through the same connection reader, play decoder,
`checked_play`, `NetEvent::Packet` queue and game-thread handling.
Both use ordinary placed initial logins, not held transfer arrivals.
This establishes the same source path for each matched movement packet.
It does not establish identical ancillary packet mixes, compression work
or tracking work. Record negotiated compression, per-tile counts and actual
forwarding/tick rates. Any difference limits the descriptive direct
comparison. The strict pass line uses the complete via-proxy bound only.
The direct cell is a reference, not a new D9 capacity claim. Its proxy serves
642 sessions rather than 650, and that difference is reported explicitly.
If matched per-tile gameplay or visibility cannot be established, the direct
comparison is unproven. It cannot repair a failed via-proxy result.

Match the complete unchanged movement payload fingerprint and full player
identity in connection order, with causal decode timestamps. Preserve both
ordered streams and send indices. Do not alter packet fields to add a tag.
Any ambiguous duplicate match stays unresolved and counts as infinity, like
an unmatched send. A two-second per-send matching horizon limits ambiguity
across repeated patrol laps. Any later decode counts as infinity, not a
finite fast match or an omitted tail. This cannot make the 5 ms test easier.
Record decode-only events separately; never count them
as fabricated sends. Include all sends inside the fixed measured window,
with a fixed two-second decode drain afterward. A later or missing decode
counts as infinity, not a removed tail. Interior witnesses avoid a changed
direct-path handover workload. Their result does not cover crossing latency.
Do not trim tails or subtract limiter pauses. Do not subtract two P95 values
and call that the P95 of added delay. The primary estimand is the complete
via-proxy send-to-tile-decode distribution. It is a conservative upper bound
on the proxy's contribution because it includes nonproxy work. Its nearest-
rank P95 must be strictly below 5 ms, with unmatched sends at infinity.
Report P95 for each of the four tiles and overall. Require exactly two
preregistered witnesses on each tile, with both visible throughout the cell.
Each per-tile result keeps its own unmatched denominator.
The direct distribution diagnoses baseline cost. Separate-cell quantile
differences are descriptive, not a per-packet added-delay distribution.
If the full via-proxy bound fails, proxy-only latency remains unproven unless
an additional valid estimator is proposed before another run.

Both paths split the same eight witnesses into four two-player native bot
drivers. The other five drivers hold the remaining 642 players. Both cells
therefore have nine bot processes inside the same aggregate one-core cap.
Explicit original indices and original cohort populations preserve every
name, UUID, seeded anchor, patrol radius and waypoint phase. No extra client
or pig is added. The placement audit checks those route inputs.

Run matched 60-second reference and via cells after 30-second warmups at
650 total players, with the same selected fixed tiles/store and proxy.
Retain bot deadlines, all histogram bins, proxy CPU and threads, matched
packet counts, forwarding rates, limiter stops and per-tile raw ticks.
No authority closure, expiry or dropped tick buffer is acceptable.
975 is supplementary stretch evidence if it runs, not a requirement here.

First measure the selected unchanged proxy, using default mimalloc only if
its memory pair qualifies. Any later runtime change gets a fresh named
variant and a comparison against that measured proxy. Timestamp hooks live
in a separate measurement bundle on FINAL-09 plus `horizon-13`; they do not
silently replace E24's fixed engine bytes. The new local patch will be
`patches/horizon-15-latency-stamps.patch`. The other Horizon checkout remains
outside E25 ownership.

Internal read-to-written histograms remain useful for comparing changes,
but exclude pre-read kernel queues and do not prove proxy-only wire latency.
E23's external 7.31 ms bound includes two taps and TCP queues.
Its internal 2.28/0.88 ms values do not replace that external result.
Use the memory pair's CPU and internal latency deltas to decide whether
another runtime change is justified. The current reactor drains its whole
ready-read set before writes; that is a source lead, not measured blame.
If needed, add bounded queue-residence/turn telemetry before changing its
write fairness. Do not replace 1,300 management threads without evidence
that their scheduling is the remaining measured delay.

## R1-based latency diagnostics

These rows use unchanged frozen reactor-1 `29f1ff02`, not mimalloc.
The measurement bundle is
`.local/bin/meridian-e25/e25-latency-r1-diagnostic/`.
Its lease base is explicitly `lease-fixed-r1`. It is not the later cadence
fix. Both complete rows have 650 real players, 256 pigs, nine native bot
drivers, zero trace errors/drops and no authority event. Both retain their
original limiter and all tails. No packet tap, handshake shim or login relay
is present.

The reader-order regression did not compile because its test connection
call omitted the port. Release, formatting, strict production Clippy,
16 bot tests and 12 clock/lease tests passed. The reader regression did not
run. The reviewer authorized diagnostic rows only. Neither row can qualify
the M4 latency item, even if its numeric bound is below 5 ms.

| Tile | Via matched/sent | Via P95 ms | Direct matched/sent | Direct P95 ms |
| --- | --- | ---: | --- | ---: |
| a | 2,404/2,404 | 4.784125 | 2,406/2,406 | 2.645458 |
| b | 2,404/2,404 | 5.002000 | 2,406/2,406 | 2.153750 |
| c | 2,404/2,404 | 5.624708 | 2,406/2,406 | 1.226334 |
| d | 2,404/2,404 | 5.205500 | 2,406/2,406 | 0.815125 |
| Overall | 9,616/9,616 | 5.135417 | 9,624/9,624 | 2.148375 |

No unmatched or ambiguous send was discarded. Each tile has its two
preregistered witnesses. Maximum full-path delays are 24.800500 ms via and
12.460500 ms direct. The via bound is close to 5 ms but does not meet it.
The direct row is a reference, not a proxy-added-delay distribution.
Subtracting these P95 values would not give added-delay P95.

| Metric | Via | Direct reference |
| --- | ---: | ---: |
| Sessions through proxy | 650 | 642 |
| Proxy CPU cores | 1.119367 | 1.035470 |
| Proxy peak threads | 1,329 | 1,314 |
| Internal serverbound P95 ms | 0.440 | 0.220 |
| Internal clientbound P95 ms | 0.750 | 0.640 |
| Server stopped wall seconds | 2.543497 | 0.906296 |
| Bot stopped wall seconds | 0.144348 | 0 |
| Largest raw tile P99 ms | 45.730416 | 34.242917 |

These are separate clocks and populations. The narrow internal histogram
does not include pre-read kernel queueing or the tile's later reader
scheduling. Their P95s cannot allocate fractions of an individual packet's
delay. Limiter totals are retained, but this runner has no per-stop interval
ledger to correlate with individual movement samples. No pause is subtracted.
Packet-matched proxy ingress/write-completion stamps would be needed to
separate those stages before blaming a queue or scheduling policy.

Both rows pass the retained D9 diagnostic checks, with no dropped raw ticks.
Every sampled tile has 65 readers and the same loaded-chunk counts between
paths. The nominal interior work and all 65 crossers are unchanged.
Minute-scale mean placed counts are not asserted bit-identical: crossing
phase and sequential census queries differ slightly. Each matched movement
uses the same tile decoder/validation/queue path. Both witness streams report
compression threshold 256 and no encryption. The small movement frame is below
that compression threshold. This does not prove identical ancillary traffic.
The descriptive comparison remains limited by 642 versus 650 proxy sessions,
different limiter exposure and the reader-test verification gap.

The published per-session ledger covers all 650 players in each row.
Via has 650 unsigned normal proxy-front connections. The reference has
642 such connections and exactly the eight preregistered signed direct
sessions. No signature or secret is stored. The private secret-file argument
is also redacted in the published command rows. Trace publication accepts
only the complete movement-packet shape, not login payloads.

Via summary: `data/E25/e25-latency-r1-via-d1-summary.json`, SHA-256
`f032798b787c351a240e8d8da26be4d5e57c6aab8c10733264c0ae5a5af221cf`.
Direct summary: `data/E25/e25-latency-r1-direct-d1-summary.json`, SHA-256
`b279a7dc2c6668c0bc9be68f9191db2b25bdcf353f13013f2147d6f322707578`.
Each links all raw movement streams, per-session ledgers, censuses and
decompressed-byte hashes. No 975-player latency row was run.

### STOP attribution and CPU profile

The original d1 rows retain STOP totals but no signal-call ledger. Their
packet overlaps cannot be reconstructed from one-second process counters.
New d2 rows add before/after brackets around the existing STOP/CONT calls
on the same absolute raw clock as the movement stamps. Budget decisions,
PID membership, credit, limits and signal arguments remain unchanged.
Final resumes are captured too. These are signal-call brackets, not kernel
acknowledgements of exactly when a process stopped executing.

The P95 tail means every send at or above that row's observed nearest-rank
P95, including unmatched sends at infinity. Both d2 rows have 481 tail sends.

| Bracket association | Via d2 | Direct d2 |
| --- | ---: | ---: |
| Send point inside server inner STOP/CONT bracket | 195/481, 40.5% | 37/481, 7.7% |
| Send-to-decode span overlaps server inner bracket | 311/481, 64.7% | 42/481, 8.7% |
| Span overlaps outer signal-call envelope | 311/481 | 42/481 |

The two P95 thresholds differ. A common fixed 5 ms threshold gives 508
via sends and 90 direct sends; 312/508, 61.4%, and 14/90, 15.6%, overlap
server brackets. Most of the via slow tail has this association, but a
substantial remainder does not. Proxy and target-tile brackets align because
the same server budget pauses both. This is evidence for the CPU/limiter
lever, not proof that one component causes every delayed packet.

Overlap readbacks are `data/E25/e25-via-stop-overlap-d2.json`, SHA-256
`9d3519875d1a90ccc2fc48b62ba4d558927145125c420fef6095e69da406a26f`,
and `data/E25/e25-direct-stop-overlap-d2.json`, SHA-256
`26119ff08b10da3efe11baedaa9b99aaee55c7d59bd9963fbea4ad0fdc3f2e96`.
The fixed-line readbacks are stored separately with `five-ms` in their names.
No pause is removed from any latency sample.

Native thread endpoints from the first via row cover 96.23% of proxy CPU.
Four IO drivers use 0.842370 core. The 650 terrain managers use 0.233741,
including 0.196538 system core. The 650 player supervisors use only about
0.000097 matched core. Thread count alone therefore does not identify the
CPU lever. Manager CPU includes genuine control/reader work as well as
timer/wakeup costs. Context-switch counts do not give CPU seconds.
Readback: `data/E25/e25-thread-cost-r1.json`, SHA-256
`d8815c13634cf2429da33d7196143ae6546a8496591c29a3c5ca5555b7ab87b7`.

A 10-second Time Profiler capture runs after the d2 latency metrics are
frozen, with the same 650 players and budgets still active. It cannot
contaminate the earlier 60-second timing window. Profiling itself remains
perturbed diagnostic work. The local trace is under
`.local/evidence/E25/e25-latency-r1-via-brackets-d2/proxy-profile/`.
The capture yields 11.107 Running CPU seconds. Blocked occupancy is excluded.

| Exclusive Running group | Share |
| --- | ---: |
| Socket syscalls | 39.88% |
| Readiness, synchronization and wakeups | 23.01% |
| Allocation and byte copies | 11.46% |
| Ghost view and packet rewrite | 6.25% |
| Framing and compression | 3.60% |
| Clock reads | 2.72% |
| Control and JSON | 2.16% |
| Terrain work outside those leaf costs | 1.74% |
| Other/unattributed | 9.18% |

Top leaves include `__recvfrom`, `writev`, `__semwait_signal`, `kevent`,
`mach_msg2_trap`, `mach_absolute_time` and tiny allocation/free/copy helpers.
These are heuristic exclusive groups, not precise function boundaries or
an exact charge for context switching. Readback classification v2 explicitly
puts Running `semwait` under wakeups rather than its terrain caller; the
original capture and earlier classification are retained unchanged.
`data/E25/e25-proxy-profile-via-d2-r2.json`, SHA-256
`85f8ba52ad698db5fc9fb17d4966a3b47ec1c4296023266f91003a4cc7d6f1a8`.

### Ready candidate diagnostics and next lever

All these rows are r1-based diagnostics with the reader-test limit. The
strict preregistered line is overall P95 below 5 ms. Worst-tile P95 is
reported beside it, not substituted for the line or hidden.

| Proxy / row | Proxy cores | Peak threads | Overall P95 ms | Worst tile P95 ms |
| --- | ---: | ---: | ---: | ---: |
| Reactor-1, bracketed d2 | 1.056282 | 1,331 | 5.138417 | a, 5.995750 |
| Mimalloc d1 | 1.050497 | 1,330 | 7.959417 | a, 9.131667 |
| Mimalloc d2 | 1.026603 | 1,332 | 7.997709 | a, 9.203833 |
| Scheduler-2 d1, fixed-window readback | 0.971254 | 682 | 5.121833 | d, 5.182667 |

Both mimalloc rows retain every selected send, with no authority event.
The large tail regression repeated without a useful CPU margin. Under the
reviewer's condition, mimalloc is disqualified as the memory fix. It cannot
trade latency for RSS. No purge retuning or 30-minute mimalloc pair is started.
The repeat summary is `data/E25/e25-latency-r1-mimalloc-d2-summary.json`,
SHA-256
`a8b45e349baa24d8be4b5f4ebeb2865faeeda8f3f834e7bc214b0a6f57d822a0`.

Scheduler-2 removes most terrain-timer threads and saves about 0.09 core in
this comparison. It tightens the worst tile but does not create overall
latency margin here. Its binary `8716c34f` was verified against reactor-1
plus exactly `meridian-07-terrain-scheduler.patch` and
`meridian-07-scheduler-stop-race.patch`, checking all 249 source files and
the frozen release gates. It is the reference for the next bounded lever.
Proof: `data/E25/e25-latency-scheduler2-r1-diagnostic-provenance.json`, SHA-256
`ad0fd20a01c722aeaffd23b01f4efed1ef874b4a0412bccae30929006a2de5eb`.

The original scheduler collector stopped on partial bot trailers written
during teardown. The fixed window was recovered read-only, not rerun.
Exact raw-clock boundaries persist in every census sample. Both partial
trailers are at least 187 ms after the fixed drain boundary: one carries
its timestamp, the other follows a later complete row from the proven
single-threaded, monotonic bot writer. No measured record or tail was
removed. Original bytes and failed summary remain intact. Fresh readback:
`data/E25/e25-latency-r1-scheduler2-d1-readback-summary.json`, SHA-256
`f73ed127b856e85e9572174dfbbe8b7e31c9df8081f42438a69ecb44c4681183`.
Future collection accepts a truncated trailer only with this explicit
post-drain proof; an unknown or in-window record remains a failure.

Only 117/481 scheduler P95-tail spans overlap server brackets, 24.3%.
The rest still need queue/readiness investigation. STOP totals alone are
not a decomposition. A single authorized scheduler-2 server-uncapped row
is queued to separate the limiter counterfactual. It retains the bot cap
and every other fixture input. It is explicitly not D9-valid and not M4
evidence, irrespective of its numeric P95.

That authorized uncapped row completed. It is not D9-valid, not M4 evidence
and still has the r1 reader-test verification limit. Only the server limiter
was disabled. The bot limiter stayed at one core. Every one of 9,616 selected
sends matched, with no authority event or binary change.

| Scope | Overall P95 ms | Worst tile P95 ms | Proxy cores | Peak threads |
| --- | ---: | ---: | ---: | ---: |
| Scheduler-2 capped d1, diagnostic | 5.121833 | d, 5.182667 | 0.971254 | 682 |
| Scheduler-2 server-uncapped d1, not D9 or M4 | 3.288625 | a, 5.117000 | 0.919124 | 681 |

Uncapped tile P95 values are a 5.117000, b 3.968916, c 2.823500 and
d 2.156000 ms. The finite maximum is 12.986625 ms, with no trimmed tail.
Server STOP time is zero. Bot STOP time is 0.023136 s across three cycles.
The 481 P95-tail packet spans overlap no server or bot STOP bracket.
Aligned sampled steady server CPU is 3.198118 cores, without a cap.
An average below four does not exclude short bursts that would trigger the
unchanged limiter. Start load is 3.827; maximum measured outside process
average is 0.09544 core. This is a counterfactual, not proof that the capped
D9 path can meet the line. No quantile subtraction estimates proxy-only work.

Summary: `data/E25/e25-latency-r1-scheduler2-server-uncapped-d1-summary.json`,
SHA-256 `77f458f9a92c047e5852bb3e23f6b537dd39620035b59d0f1c78b59ab4486281`.
Raw decompressed SHA-256:
`17942f6da95abfa09acbe59b85272e186c5c2bed12501c8ef3614171023ff0ef`.
Overlap readback: `data/E25/e25-scheduler2-uncapped-stop-overlap-d1.json`,
SHA-256 `458175758e2bbdb28c0260498cc16c8b39bb7de3ac35de66d39d9089bde06d37`.

### Combined CPU diagnostic, planned

The r2 row order remains reactor-1, scheduler-2, then scheduler-2 plus
writev. Once E26 freezes its qualified patch-10 warming tile variant,
run one separately named combined diagnostic with scheduler-2 plus writev,
the r2 stamp hooks and those patch-10 tiles. Use the same 650-player via
cell and compare it with scheduler-2 on r2. Record E26's exact patch,
binary hashes, base stack and gates before combining anything. E25 does
not use or change the Horizon-tiles checkout.

The uncapped mean of 3.198118 server cores is below four. That does not
exclude bursts above four, and it does not reproduce the capped burst
distribution. Reducing either proxy or tile CPU may reduce limiter pauses;
the combined row tests that hypothesis without changing the limiter.

Tile a is worst in the uncapped row. Current stamps cannot isolate its
decode-side share. The bot stamps before writing; the tile stamps after
complete frame decode and before the main-thread queue. There is no paired
tile decode-start or socket-read-start stamp. Subtracting internal histogram
quantiles would not recover that missing boundary. Report tile a's full
send-to-decode distribution separately, without calling it decode CPU time.

The new `meridian-11-writev64-r1.patch` is prepared against scheduler-2.
On this Mac, IOV_MAX is 1,024. The candidate packs up to 64 FIFO writev
descriptors instead of 16, using a fixed stack array instead of allocating
a descriptor Vec. Other platforms retain the old 16-descriptor limit.
The 256 KiB turn, 8 MiB pending-byte cap, 8,192-chunk cap, callback order,
partial-write offsets and backpressure remain unchanged. No timer is added.
This is a hypothesis from the measured syscall/allocation costs, not a gain.
A rebuilt scheduler reference and the candidate use matched compiler,
original dependency relocation and adjacent via measurement cells.

The first writev build failed before producing a binary. Its helper and
tests referred to `Chunk`; the existing queue element is `WriteChunk`.
Source, dirty status and exact index restoration passed. Failure record:
`data/E25/e25-scheduler2-writev64-r1-build.json`, SHA-256
`c3375cd226c06990f2e50d04fe046b5a7f5c661b8271e9111d779185ee1145e7`.
The original patch is retained. The new
`patches/meridian-11-writev64-r2.patch` changes only that type name,
SHA-256 `7a384e50f35868d43ed2dddec891729fc7cafe8dc4881caa5b806b5b761588bd`.
Private-index application checking passes. A fresh r2 build is queued.

Qualification requires an r2-based stamp bundle with every gate passing,
including the reader-order regression. Each selected candidate needs two
via repeats, both with overall P95 strictly below 5 ms, and per-tile/worst
values beside the overall result. No r1 row can close the M4 item.

### Timestamp output, test correction and handback

The r1-based measurement release is frozen at
`.local/bin/meridian-e25/e25-stamps-r2-release/`.
Engine SHA-256 is
`0028489388e8cd02f394034d9cd8afd3d68709f399232738754c520c3ad06541`.
Bot SHA-256 is
`de8f997f86e92171dbd47f75a4f94d1fed98f82e8450ca88572e77d87971e045`.
Its explicit verification-limit record is `data/E25/e25-stamps-r2-release.json`,
SHA-256 `f4c6f5f864235e40eb5cce01a16e41661c3cc93a23262e68773264b1119be763`.

The compiled r2 stamp patch is
`patches/horizon-15-latency-stamps-e25-stamps-r2.patch`, SHA-256
`7d89061bd244b7c4063a9544704c93b9eb6f5c809de7da9ba177e494f29324a2`.
Its tile hunk is `lan/io_pool.rs::Conn::read`: raw capture at archived line
537, before `session.packet` at 547. Its bot hunk is
`measured.rs::Bot::flush`: raw capture at 605, immediately before the write
at 609. Line anchors refer to the retained compiled source archive, not
another builder's later checkout.

The test-only call is corrected in the new r3 patch, without checkout writes:
`patches/horizon-15-latency-stamps-e25-stamps-r3.patch`, SHA-256
`cdbc4a388f703007a0b4e7bfe6acf320b0a9fd93f6b947d4f5c590decf7c00a9`.
It passes application checking against the retained private H13 r1 index.
The correction is now selected on the frozen cadence-r2 lease source.
Its build and gates are queued. No pass is claimed before every gate,
including the reader-order regression, completes.

Horizon was restored and handed directly to E24. All 5,586 source files and
dirty status were checked. Staged entries, flags and resolve-undo were equal
before restoring the canonical exact index image `020d12c7`.
Proof: `data/E25/e25-horizon-handback-r1.json`, SHA-256
`b60aeec5f72999c0782862eb6d19cc1a4402ba017a1551dbe9fcf395c22b982c`.
E24 subsequently returned the restored checkout after freezing cadence r2.
E25 owns it for the r2 stamp build. No lease-rule change is part of E25's stamps.

The r2 input bundle is `.local/bin/goliath-e24/lease-fixed-r2`.
Its engine SHA-256 is
`53b0918027a700d9c2684936beb218c922b229401b7a3344805d65831d0adb9f`.
Its store SHA-256 is
`9636fa6d8089b071be241437e79eb2942a07dc756039b394e73cc8f705e26d0e`.
Its FINAL.json SHA-256 is
`4914d701106683002a178cfbbf45248de2e95f44f6f269766853123e31dd5af0`.
The selected source is FINAL-09 plus only the self-contained
`patches/horizon-13-lease-renewal-r2.patch`, SHA-256
`f2c03265ceced9a3e8096bc7ee52f2d0da5b41f6c7fd79f9f845125b4cae389b`,
then the corrected r3 stamp patch. Internal sources come from E24's frozen
archive, SHA-256
`1d7a6e641e1b2def4583abc7acffd3123eb8f3ec51ed02a9d4ad28380fee9df0`,
not the live workspace. Availability acceptance is still pending.

### Coordinator waiter migration

The reviewer changed build admission while two E25 shells were waiting.
Neither had started Cargo. Their only children were sleep processes.
E25 stopped those shells, let both helpers restore their source and index,
then resumed the exact source selections and unchanged Cargo arguments.
Fresh attempt names are `e25-stamps-lease-r2-b2` and
`e25-scheduler2-reference-b2`. Failed prestart records remain intact.
Both replacements appear under `builds waiting` in the new coordinator.
The uncapped quiet waiter is unchanged. No measurement was interrupted.
Record: `data/E25/e25-coordinator-waiter-migration-r1.json`.
Its SHA-256 is
`e963a4fe58b0bd124b7f78bf5ed7397db1bc4d7abcf05f576e21277e3b9fbe22`.

## Evidence and verdicts

### Rebuilt control

The control is frozen at `.local/bin/meridian-e25/e25-control-r1/meridian`,
SHA-256 `80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378`.
It is not bit-identical to frozen reactor-1,
`29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b`.
All 248 source inputs and the complete dependency resolution match the
frozen provenance before one store dependency path is relocated. No package
was added or updated. The relocation changes Cargo's path-package identity
and build metadata, so byte identity is not expected. This does not isolate
the binary difference to a particular linker byte or prove reproducible builds.
The original E18 final build selected proxy, sidecar and packet-client targets;
E25's build selects the proxy executable. Both E25 variants use that same
target selection and the same relocated internal source, not the live tree.

The compiler is Rust 1.98.1, commit `48a229ceaefd4985c50990b14116b6d856af0985`,
native `aarch64-apple-darwin`, LLVM 22.1.8. The profile is release, ThinLTO,
one codegen unit, debug 0, incremental 0 and six build jobs for both variants.
Formatting, strict release Clippy and scoped release tests passed.
Tests passed 96 handoff, 104 net, 562 proxy and two signal cases.
The same known listener-burst test was explicitly skipped, as in E18's gate.
One existing net integration test was ignored. Neither exclusion is a new pass.
Source, original dirty status and Git index restoration passed.
The exact commands, source hashes and local log hashes are in
`data/E25/e25-control-r1-build.json`.

### Default mimalloc build

The candidate is frozen at
`.local/bin/meridian-e25/e25-mimalloc-default-r2/meridian`, SHA-256
`a1b43b65ae9c0fcd66de8a11d08960cfbe88395e27884287211c9708bce7a4f2`.
Release, formatting, strict Clippy and the same scoped release suites pass.
The same baseline skip and ignore remain explicit. Source, dirty status
and exact Git index restoration pass. Only the allocator manifest, lock and
global selection differ from the rebuilt control. Compiler, settings,
original dependency archive and frozen patch stack are equal.
The only added packages are the two pinned allocator wrappers above.

The complete patch is `patches/meridian-10-mimalloc.patch`, SHA-256
`c14b038aec1e8dcf918a7ea537962465c286886801af99d9739805a97188c07b`.
The candidate build record is `data/E25/e25-mimalloc-default-r2-build.json`,
SHA-256 `1c19fca28137130ce45ddfcad9bae4b219e9e10d04ba74476c4e52da74af2500`.
The control build record SHA-256 is
`88a37dff7abbf1c8c8d5448f9f7c4fae10d27a5f76e67226a34bfcb165ec6765`.
Clean application on the seven-patch frozen stack and equality of all 248
compiled inputs pass in `data/E25/e25-patch-proof-r1.json`, SHA-256
`e241c2e22877479a95588f76e24a8fa2e28fad2aa03bd54f74526b73f38d3466`.
This is build qualification only. No valid matched memory pair completed.

### Preparation and independent readback

The read-only reconstruction matched all 248 frozen source files and every
ordered patch hash. Nine restored checkout files differ from reactor-1.
They remain untouched during preparation. The original dependency archive
comes from `git archive a7988a4` with workspace manifest, lock and lint config.
Its SHA-256 is
`d77a2bfe517ee29f64ba5bf7c44ad507a0527196d2ca41a5de41cb0b208bb911`.
Every selected internal source file matches reactor-1's canonical hashes.
The preparation record is `data/E25/e25-preparation-r1-build.json`.

Independent readback re-parsed the three retained native `vmmap` summaries,
verified their recorded hashes and recomputed the delta arithmetic.
It also recomputed RSS and thread/mapping ranges from all nine native points.
The readback is `data/E25/e25-native-readback-r1.json`, SHA-256
`e5d98f9d226ac921b070121920a67dae42d680be43fcb4dedb0a30a47421cb46`.
Forty-eight resource-free E25 tool contracts pass. They cover patch application,
exact pins, private-path rejection, lossless tick merging and strict memory
endpoints. Python syntax checks pass.
The latency contracts also prove exact-5-ms rejection, unmatched infinity,
ambiguous-duplicate refusal, per-tile coverage and conservative late tails.
The route and session-ledger contracts keep all 650 indices, the same nine
drivers and original proxy handshake hosts. Signing a non-witness is refused.
The read-only Rust layout audit found zero missing module ownership docs
and zero undocumented unsafe blocks in
the current internal source. E25 changed no internal Rust crate or lint rule.

The first mimalloc attempt failed before any Cargo command. Its template
hunk declared six old lines but contained five. Source, dirty status and
index restoration were independently rechecked against the backup.
The error serializer also rejected a private path in the exception, so a
separate retained preflight record documents both failures:
`data/E25/e25-mimalloc-default-r1-preflight.json`.
The corrected template now has an application test against a disposable
copy of the frozen private index. The successful retry has a fresh r2 name.

### Data rules

All E25 data names start with `e25-` or `repro-`.
Whole-host rows label outside processes `outside` while retaining counters.
Raw files over 1 MB use `.json.gz`; recorded hashes refer to decompressed
bytes. Native traces, screenshots and environment dumps stay under
`.local/evidence/E25/`. Frozen binaries get fresh directories under
`.local/bin/meridian-e25/`. No frozen directory is overwritten.

### Current handoff

The original `e25-memory-fixed-r1` pair used the verified
`.local/bin/goliath-e24/lease-fixed-r1` fixture. Its control completed but
failed raw tick P99; the candidate was preflight-only. Tile SHA-256 is
`abb3aecf8da2a5afdcb3998188e4d82a858ea6c33acc4e56fc332de206aa53ec`.
Store SHA-256 is
`71e7840cf71fab15871d9b19ebf446bf65e7826a15b0c7468e4bca6023765fe5`.
The fixture's FINAL.json SHA-256 is
`59fc1bc6c677bc1b3ac20df3a6bf178d393c0e840bf7495869cf3f7f3230bf63`.
All recorded root/scoped gates have exit zero. The original index handoff
hash `020d12c787932d58d659444ed9d50e98c2f182393757605b3d0312030a275b25`
was independently read back before selecting Horizon source.

E24 subsequently reported that its first H13 soak failed before the endpoint
on a genuine tile-b keeper deadline, with zero raw-clock anomaly/resample
closures. The bundle remains byte-frozen but is not availability-qualified.
Its reported renewal and store/barrier maxima are event leads, not a disk
attribution. E25 does not change leases or silently retry. Any authority
closure in either memory cell voids the entire pair.

The pair uses fresh bundles `e25-fixed-control-r1` and
`e25-fixed-mimalloc-r1` under `.local/bin/meridian-e25/`.
Its preregistered binary/manifest ledger is
`data/E25/e25-memory-fixed-r1-plan.json`, SHA-256
`8fcbd0e755b582edc4b6f9f8edf4c9f50b66ce021f6cd603374ed8f3fcec66af`.
No memory pass is claimed. The r1-based latency diagnostics above do not
qualify M4. Mimalloc's repeated latency regression disqualifies it as the
memory fix. The frozen binaries remain unchanged. The corrected stamp patch
is queued for r2-based gates before any qualifying timing row.

Plain `scripts/clean-builds` completed and freed 0 MiB. The active Horizon
target was skipped. The Meridian target is 1,688 MiB, below its cap.
That cleanup predates the newly queued r2 stamp and scheduler builds.
The uncapped diagnostic has now completed and released its window.
The rebuilt scheduler reference passed all gates and restored source/index.
Its frozen proxy is
`.local/bin/meridian-e25/e25-scheduler2-reference-b2/meridian`, SHA-256
`c9cd5891186693a598b22f70bd5ba0f927f68dd7244cc3530cea848039c108ba`.
Build record: `data/E25/e25-scheduler2-reference-b2-build.json`, SHA-256
`002fa451f8e9524bfca19bb307615eddbed7f835e0e6aa61abd1f23dbc732ef6`.
This is not the old frozen E20 binary hash. The selected 249 source files
match E20's scheduler-2; E25 relocates the original dependency path and
rebuilds with its recorded compiler/settings. Timing rows name the actual
executable rather than borrowing the earlier diagnostic binary's result.

The matched-target reactor-1 build passed every gate and restored the source
and exact index. Its frozen binary is
`.local/bin/meridian-e25/e25-reactor1-matched-r2/meridian`, SHA-256
`80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378`.
Its build record SHA-256 is
`fe72bacba1c70c813a1ac95dcb32f02c18965663bffbd2d946c68f5027ada1b9`.
The hash matches the earlier rebuilt control even with matched release targets.

The r2 stamp build passed all seven gates, including 13 clock/lease tests,
16 bot tests and eight reader tests. The ordering regression is
`a_decode_stamp_precedes_session_packet_dispatch`. Source/index restoration
passed. The engine is `33a74ea5` and the unchanged stamp bot is `de8f997f`.
Full hashes and commands are in `data/E25/e25-stamps-lease-r2-b2-build.json`,
SHA-256 `080446741cbf9cda78aefc3a7a44ce97d107482c66d91d045f423ae08384804b`.

The paired-session runner is `scripts/e25-latency-session`. It validates
the complete r2 stamp provenance, matching non-proxy binaries and proxy
compiler/settings/target selection before acquiring a window. It runs
reactor-1 then the candidate as unchanged capped via cells in that one
window, releases it, and only then compares their P95 values. The approved
memory entry comparison is candidate P95 no worse than reactor-1. It is
separate from the unchanged strict final latency line. Authority events
or incomplete/infinite comparisons do not establish entry.
The resource-free tests cover that separation, fixture drift and uncapped
or verification-limited refusal. Current memory-window ownership is recorded below.

## R2 variance and read-only closeout of latency tuning

The first all-gated r2 reactor row is D9-valid but fails strict latency.
All 9,616 selected sends match. Overall P95 is 11.159291 ms; worst tile c is
11.612541 ms. Other tiles are a 11.448750, b 10.687792 and d 10.874875 ms.
Proxy CPU is 1.119717 cores with 1,330 peak threads. All raw tick P99 values
are below 41 ms, with zero drops. No authority event was observed in its
complete traces. Summary: `data/E25/e25-latency-r2-scheduler2-p1-reactor1-summary.json`,
SHA-256 `2666e052165d7bf53fcd979aafcdb9b078e57e0def6d57ddcf6dc5215200bc5b`.

The following scheduler-2 row finished its packet collection in the same
window. Its raw D9 metrics pass. All 9,616 sends match. Timing readback P95
is 6.512042 ms; worst tile b is 9.257666 ms. Other tiles are a 8.209958,
c 3.138750 and d 3.117250 ms. Proxy CPU is 0.934872 cores, peak 680 threads.
It is not qualified: the authority collector failed during teardown.
The original failed pair is retained at
`data/E25/e25-latency-r2-scheduler2-p1-comparison.json`, SHA-256
`69ca8c60836c7b03668c8fe0e22d637ed066cf9792775665d49d021dddd52f02`.
No row was rerun or silently repaired.

### The 31-byte record

The exact final bytes are:

```text
{"at_ns":134535215125,"data":{"
```

This is incomplete JSON, ending inside the first data-key string. It has
no event name. It cannot be classified as a new renewal or cadence event.
The collector has no event whitelist: JSON decoding fails before event
selection. An event-table change cannot restore missing bytes.

The other 2,641 records in that file parse and contain no stop or anomaly.
The original collector and an independent streaming reader both encounter
the same final fragment. Raw SHA-256 remains
`58a88df9fa357870bda1378686ab92d7e06c39d1e39ca8ffd3284e4e8468a5dc`.
The row remains explicitly unproven. Classification and both readbacks:
`data/E25/e25-latency-variance-readback-r1.json`, SHA-256
`d830288809967d1fc1513cb8deb15d1244518db1cf0c906a67ff3b6fa294db3a`.

### Variance and pauses

STOP spans below are signal-call brackets, not kernel acknowledgements.
Each measured P95 tail has 481 sends. Server and bot overlap can coincide.

| Via row | Overall P95 ms | Server STOP s / cycles | Bot STOP s / cycles | Tail server span | Tail bot span |
| --- | ---: | ---: | ---: | ---: | ---: |
| r1 reactor d1, diagnostic | 5.135417 | 2.543497 / 292 | 0.144348 / 19 | not captured | not captured |
| r1 reactor bracketed d2, diagnostic | 5.138417 | 1.238118 / 140 | 0 / 0 | 311/481, 64.7% | 0 |
| r1 scheduler-2 d1, diagnostic readback | 5.121833 | 7.798413 / 974 | 0.381866 / 54 | 117/481, 24.3% | 33/481, 6.9% |
| r2 reactor p1, D9-valid | 11.159291 | 3.365930 / 391 | 0.447328 / 60 | 424/481, 88.1% | 39/481, 8.1% |
| r2 scheduler-2 p1, unproven | 6.512042 | 1.891696 / 236 | 0 / 0 | not retained | not retained |
| r1 scheduler-2 server-uncapped, not D9 | 3.288625 | 0 / 0 | 0.023136 / 3 | 0 | 0 |

For the r2 reactor, exclusive counts are 385 server-only, 39 both,
zero bot-only and 57 neither. The high reactor tail is predominantly
associated with server pauses, not extra bot CPU. R1 via-d1 has only
pause totals; its overlap cannot be recovered. The bracketed r1 repeat
is shown separately, never substituted for it.

STOP wall totals do not monotonically order P95. In particular, r1
scheduler-2 has more STOP wall time but much less tail overlap than the
r2 reactor. Pause placement and queueing matter. These rows do not prove
that every slow packet is caused by the limiter, or identify why the burst
pattern changed. No STOP time or quantile is subtracted from the pass line.

### Aligned process CPU

All values below use the same cumulative-process-CPU method and the steady
phase boundaries, not mixed native and ps clocks. Values are mean cores.

| Row | Bots | Shims | Proxy | Tile a | Tile b | Tile c | Tile d | Server total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| r1 via-d1 | 0.4448 | 0 | 1.1194 | 0.5618 | 0.5417 | 0.5709 | 0.5804 | 3.3848 |
| r2 reactor p1 | 0.4453 | 0 | 1.1199 | 0.5656 | 0.5553 | 0.5578 | 0.5618 | 3.3713 |
| r2 scheduler-2, timing-only | 0.3842 | 0 | 0.9353 | 0.5617 | 0.5298 | 0.5542 | 0.5512 | 3.1432 |
| Reviewer r2 frontier 1 | 0.3932 | 0.6064 | 1.0789 | 0.5781 | 0.5525 | 0.5734 | 0.5733 | 3.3678 |
| Reviewer r2 frontier 2 | 0.4030 | 0.6202 | 1.1052 | 0.5979 | 0.5600 | 0.5769 | 0.5788 | 3.4304 |

There is no mean CPU increase from r1 via-d1 to the r2 reactor. Those rows
both use nine bot drivers and no shims. Frontier rows use five bot drivers
and five shims. Their helper CPU is reported, not charged to the server
total or treated as a matched latency path. The retained r1 via-d1 bot
STOP value is 0.144348 s, not about 0.02 s.

### Bundle diff

Both complete 5,588-file source reconstructions match their recorded bases
and stamp patches, including the instruction-file alias. The earlier
readback omitted that alias; it is retained and superseded, not overwritten.
Compiler and build settings are equal. The bot executable is bit-identical.

Outside the stamp test, Horizon changes are only the approved H13 r2
`cluster/lease.rs` and `cluster/lease/clock.rs`. The IO-pool change is the
test-only three-argument connection constructor. No packet-path edit was
found. The internal archives also contain the approved model/store renewal
schedule, RPC deadline and memory changes. Both dependency relocations and
the r1 lock resolution were checked explicitly.

Full CPU, source and span readback:
`data/E25/e25-r2-latency-jump-readback-r2.json`, SHA-256
`ad071eb2fd44454319407028d1a78bfa7356fc5aa5d2c60d20a603097808aae6`.
This narrows the explanation to temporal burst/queue behaviour, not a
stray packet-path change or higher mean CPU. The burst cause is not proven.

### Decision and memory-first continuation

The reviewer stopped latency tuning. M4 latency is not met under D9.
The 3.288625 ms uncapped scheduler diagnostic remains not-D9, not M4.
The remaining lever is lower total server CPU bursts, not proxy work alone.
Only the final memory winner gets two strict D9 via rows and one uncapped
diagnostic. All results will be reported, including failures.

The approved memory entry rule is now the observed reactor range of
5.1 to 11.2 ms in a strict D9 via row, with proxy CPU no higher than
reactor-1. A single-session P95 comparison is no longer an entry rule.
The memory winner is selected separately from strict latency success.
Plan revision 3 is `data/E25/e25-memory-next-plan-r3.json`, SHA-256
`6999afb00ff09961cb61e8ef8fe94cc1823bb2935a1d3d4f62927db149ad8b2b`.
Earlier plans remain unchanged.

The full reactor control is running as `e25-soak-r2-control-m1` on the
unstamped lease-fixed-r2 fixture. Its preregistration is
`data/E25/e25-soak-r2-control-m1-plan.json`, SHA-256
`99490d5a1e5fdb5c3b91efc232d2a44c6450712a11fda5ce707d8039efb7786c`.
No scheduler memory cell starts by inference from the incomplete r2 trace.
The reviewer explicitly accepted the earlier r1 scheduler D9 row for memory
entry: 5.121833 ms and 0.971254 core. That row uses frozen proxy `8716c34f`.
The soak uses rebuilt proxy `c9cd5891`, with provenance-equivalent source
but different bytes. Both are reactor-1 plus the same two patch-07 files.
This substitution does not qualify the incomplete r2 timing row. The rebuilt
binary must supply its own full authority and clock ledger in the soak.
Decision record: `data/E25/e25-scheduler2-memory-entry-r3.json`, SHA-256
`50721432794612a32d690ebc90394fe3c0149704d52072533d6cd840ff0b25cd`.
No fresh S2 entry row or new latency-tuning load is queued.

The scheduler soak command waits on the control runner's exit event. It
then requires the completed control readback, unchanged non-proxy binaries
and the identical executed soak adapter before requesting its own window.
It never overlaps the control. A failed control stops the chain without retry.

The corrected writev candidate passed every scoped release gate and restored
source/index. It is ready but unmeasured, to be used only if scheduler-2 and
zone relief have not passed memory. Frozen proxy:
`.local/bin/meridian-e25/e25-scheduler2-writev64-r2/meridian`, SHA-256
`175143aec2bade26dea20d4a1d214f9ce74f37eba856ea4fcf2c63af3b0e7aca`.
Build record SHA-256:
`ee915537f2505a7b049bf089e714a959424f47e3b2f30d8f4b0d3c924a9d75f6`.

## Full soak-harness control m1

The 30-minute control completed on the unchanged native lease-fixed-r2
fixture. Rebuilt reactor-1 `80f8953a` rose from 563.750000 to 574.718750 MiB,
1.945676%, at 299.628 and 1799.757 s. Every other judged process is under
10% too. This passes the RSS soak lines but does not reproduce frozen
reactor-1's earlier 16.45% growth. It is not an allocator-fix claim.

The full authority and clock ledger passes, with no stop or anomaly.
All 65 cargo items are conserved. There are 29 owner audits and no owner
violation. Every tile runs at about 20.000 TPS. Proxy CPU is 1.203703 cores,
peak 1,333 threads. Store fill lead is only 31.099389 s before minute 5,
explicitly fragile.

Long-window raw tick P99 is a 67.698708, b 61.278542, c 59.140500 and
d 58.084084 ms, zero drops. The unchanged D9 capacity gate fails separately;
this is not a capacity-frontier pass.

Result: `data/E25/e25-soak-r2-control-m1-e25-result.json`, SHA-256
`35dfe72a0fb40471a3aa833d01beb889ddcc993a26ef69d81a1c48aee44ea4d0`.
Raw decompressed SHA-256:
`659f85450ddb14f85182dcee49aabde50d5f14a139718cc3a7cabbea3a42f9f9`.
RSS series decompressed SHA-256:
`d440a600d0c4a5f283c6871eec9ff228b00be81d3466536fbbada45f1ce83188`.
The scheduler soak began only after this run and its readback exited.

### Frozen versus rebuilt reactor provenance

Readback: `data/E25/e25-reactor-build-provenance-diff-r1.json`, SHA-256
`f65ab39b9b773d0bb7d0619ece48000befd3e9fc78e2fd6bfe3e034f689af0b6`.
It builds and runs neither binary.

| Input | Frozen 29f | Rebuilt 80f |
| --- | --- | --- |
| Profile | release | release |
| LTO / codegen / incremental | retained helper sets thin / 1 / 0 | explicit thin / 1 / 0 |
| opt-level / debug assertions / panic | effective historical values not recorded | no workspace override; release defaults after clearing profile environment |
| Debug | effective historical value not recorded, no DWARF segment | explicit 0, no DWARF segment |
| RUSTFLAGS | inherited, effective value not recorded | removed, including encoded flags |
| Target selection | proxy, sidecar, e4-client; no feature flags | identical command |
| Target / minimum OS / SDK / linker | arm64 macOS / 11.0 / 26.5 / 1267.0 | identical |
| Embedded Rust commit | `48a229ce` | same, matching recorded Rust 1.98.1 |
| Cargo.lock | 164 packages | byte-identical, no version or dependency change |
| Internal source revision | `b4dc5af0` canonical | `a7988a42` archive; all 24 compared inputs byte-identical |
| Meridian source | pinned seven-patch stack, 248 files | same, except documented dependency path |
| Binary size | 6,572,016 bytes | 6,572,016 bytes |
| Allocator | System default in verified source | same; no added allocator package or declaration |

`otool -L` reports the same libSystem, libobjc, Foundation and libiconv
paths and versions. Allocator imports are the same. No hash-bound old
fingerprint or full historical feature/flag environment survived. Identical
manifests and commands support equivalent feature selection, not proof of
unrecorded flags. Historical runtime allocator environment is also unknown.

No known dependency, allocator, source or linked-library difference explains
the RSS contrast. One run each cannot separate a build effect from run-to-run
variance. Full attribution remains open. The reviewer approved one repeat
of frozen 29f in this same harness after zone if this diff found no cause.
The diff found no explanatory change. The reviewer approved that repeat in
advance, straight after zone without another approval round. It is not yet
queued. If 29f stays under 10%, the result supports run-to-run variation;
the memory item still rests on the final proxy's own soak and independent
reproduction. If it grows again while rebuilds do not, report an unexplained
build/runtime effect and select a passing rebuilt binary. The allocation
mechanism behind the original growth remains unattributed.

For future rows, `scripts/e25-latency-cell-r2` preserves packet metrics,
raw-file hashes and STOP brackets even if authority JSON decoding fails.
That row remains unproven, never accepted by dropping a partial record.
The old collector and all old raw files remain unchanged. A resource-free
contract checks this with the exact 31-byte fragment. This is bookkeeping,
not latency tuning or a packet-path change.

## Reviewer reproduction of rebuilt 80f

Run after the frozen-29f repeat and the second strict 80f via row.
Use a fresh name if this one exists. Queue owner is `reviewer`, not E25.
The wrapper acquires and releases that window. It waits for start load at
most four, uses the unchanged E24 soak adapter, 650 players, 65 cargo items,
minute owner audits and exactly 1,800 measured seconds. It does not retune
the limiter, planning rate, leases or receipt capacity.

```sh
caffeinate -i python3 -B scripts/e25-soak-control \
  --name repro-e25-80f-reviewer-r1 \
  --bins .local/bin/meridian-e25/e25-soak-r2-reactor1 \
  --campaign-plan docs/experiments/data/E25/e25-memory-order-r4.json \
  --owner reviewer --collect-only
```

The bundle uses native lease-fixed-r2 tile `53b09180`, store `9636fa6d`
and proxy `80f8953a`. Full proxy SHA-256 is
`80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378`.
Only owner metadata changes from the builder's adapter. Runtime workload
and sampling remain the same.

After collection releases `reviewer`, run the readback once:

```sh
python3 -B scripts/e25-analysis-exec python3 -B scripts/e25-soak-readback \
  --name repro-e25-80f-reviewer-r1 --owner reviewer
```

Readback prints the exact result path and decompressed-byte hash.
The result is `data/E25/repro-e25-80f-reviewer-r1-e25-result.json`, or its
`.json.gz` form if needed. Native evidence stays under
`.local/evidence/E25/repro-e25-80f-reviewer-r1/`. Inspect the proxy RSS
endpoint line separately from long-window raw tick P99. A stop, anomaly or
incomplete authority ledger cannot be turned into a pass or silently retried.

## Scheduler failure and frozen-control repeat

The three completed full-soak cells use the same native r2 tile/store/bot
fixture and byte-identical executed E24 adapter, SHA-256
`adcb9862a1a7b36427eb4134e91a611914ed9fe0c0a97a4212c45051592025ee`.

| Proxy | Minute 5 MiB | Endpoint MiB | Growth | Proxy cores | Peak threads | RSS line |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rebuilt reactor 80f | 563.750000 | 574.718750 | 1.945676% | 1.203703 | 1,333 | pass |
| Rebuilt scheduler c9cd | 588.250000 | 788.484375 | 34.038993% | 1.029569 | 683 | fail |
| Frozen reactor 29f repeat | 583.828125 | 718.250000 | 23.024221% | 1.112995 | 1,335 | fail |

Scheduler-2 reduces CPU and threads but is a clear memory failure. It is
dropped as a base. No live-heap measurement was added, so its new growth
is not labelled a leak or attributed to allocator residency.

All three have complete passing authority/clock ledgers, zero owner
violations, all 65 cargo items conserved and about 20 TPS on each tile.
Long-window raw tick P99 still fails separately. Scheduler a/b/c/d values
are 51.632125/51.878833/48.362958/45.311000 ms. Frozen-repeat values are
57.141458/55.304500/54.425667/52.540667 ms. Both have zero dropped ticks.
Receipt-fill leads of 31.283846 and 30.245587 s remain explicitly fragile.

Scheduler result: `data/E25/e25-soak-r2-scheduler2-m1-e25-result.json`,
SHA-256 `45de73463fa0ba041db5c4935e4e9a2132756ccfb6599984e2e9db415d17d4b8`.
Raw decompressed SHA-256:
`ee7b2a4940d309e0b7eb6e0e3d93f510e6d33fd4d3a26f587db1d34c43783a16`.
Frozen-repeat result: `data/E25/e25-soak-r2-frozen29f-r1-e25-result.json`,
SHA-256 `0174bd1dc8583efaf49053da03643e61362310ac9f52c278d2d02ca68350f63b`.
Raw decompressed SHA-256:
`70938a88f14d042fa3eb33f4f3034ff90b79e4eeaca5107702df90ac14fc538d`.

The frozen proxy's growth returned. Per the reviewer's decision, report
an unexplained build/runtime effect, not a proven dependency finding or
run-to-run-variation explanation. A selected final proxy must be a passing
rebuilt binary. Full allocation/residency attribution stays open.

### Zone readiness and revised order

The reviewer moved the frozen repeat ahead of zone's soak. The zone build
finished first and restored the exact source/index. Its scoped release,
formatting, strict Clippy and tests pass, including the native call-return
unit and two scheduling tests. The non-macOS no-op unit is present but was
not executed on this host.

Frozen candidate:
`.local/bin/meridian-e25/e25-zone16m60-reactor-r1/meridian`, SHA-256
`ffe1b1f01773c6a09e4a34cf6be69a6a35dbc4de7125f967c100c30dfa2d0039`.
Build record: `data/E25/e25-zone16m60-reactor-r1-build.json`, SHA-256
`bfb6a6f85bceedffdf03970c015c59e9189809be14a327ef4bc23111562b1543`.
Only the local MIT `meridian-zone` 0.1.0 package is added. No registry
dependency or existing-crate lint changes. It remains unmeasured.

Order revision 4: `data/E25/e25-memory-order-r4.json`, SHA-256
`dfcfca3c1bf968314b6246f6d58011c0ee2957ab425e3336b66305f447d37f7c`.
Zone's soak is allowed only if frozen 29f exceeds 10% again or the reviewer's
80f reproduction fails. The frozen repeat now satisfies that condition.
Its entry check is queued, with no latency tuning. The final selection
still needs a passing own soak and independent reproduction.

## Final selection and verdict

The reviewer independently reproduced 80f on the same native r2 fixture,
65 cargo items and 1,800-second soak harness. Proxy RSS rose from
569.250000 to 583.968750 MiB, 2.585639%. All other production processes
stayed under 10%, clock checks passed, 29 audits found no violation,
cargo was conserved and all tiles ran at about 20 TPS.

Reproduction: `data/E25/repro-e25-80f-reviewer-r1-e25-result.json`, SHA-256
`92b1cf89a0f769304c3be1770f6d65fa2a9e9a231275215fe14051ac01794e7c`.
Raw decompressed SHA-256:
`a356db6855edf97edeae7104e8c2ec6b6514b0851c9a50d0d552f85ff7dfa4e5`.
RSS-series decompressed SHA-256:
`c0a406c157daab64e86d40bff591ae80c3823d9aaf3ad6e67293cbae6c585ce2`.

| Binary / run | Minute 5 MiB | Endpoint MiB | Growth | Verdict |
| --- | ---: | ---: | ---: | --- |
| 80f, builder m1 | 563.750000 | 574.718750 | 1.945676% | pass |
| 80f, reviewer reproduction | 569.250000 | 583.968750 | 2.585639% | pass |
| 29f, E24 r2 soak | 575.703125 | 670.406250 | 16.449993% | fail |
| 29f, E25 repeat | 583.828125 | 718.250000 | 23.024221% | fail |

Memory verdict: 80f passes and has independent reproduction. Retire 29f
from the selected stack. This is not evidence for a dependency or allocator
fix: no such change was found. Report an unexplained build/runtime effect.
The original allocation/residency mechanism remains open. Long-window raw
tick P99 remains separately reported, not reclassified as a capacity pass.

Selected immutable proxy:
`.local/bin/meridian-e25/e25-reactor1-matched-r2/meridian`, SHA-256
`80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378`.
Ready native soak bundle: `.local/bin/meridian-e25/e25-soak-r2-reactor1/`.
The selected source is reactor-1's pinned seven-patch stack, with only the
recorded dependency relocation during building. It has no new allocator,
zone crate, scheduler patch or writev patch.

### Zone cancellation

The reviewer selected 80f after reproduction and cancelled zone. The entry
had already entered its timed interval before that instruction arrived.
E25 stopped it immediately with SIGINT, and cleanup released the window.
The attempt retained 45 partial samples, no endpoint and no P95. No zone
soak or completed entry result is claimed. Calling it entirely unmeasured
would conceal those partial samples, so they remain explicitly recorded.

Cancellation record: `data/E25/e25-zone16m60-reactor-entry-r1-summary.json`,
SHA-256 `65e04976718032526ff151cbdeba9ec111741cb44d3e69ea2ec11e1fdc972d8c`.
Partial raw decompressed SHA-256:
`e5dd702bebe01c0015ff4fdd3f3cc6b49047f77e72c88027034cbfd242b687e9`.
The frozen zone candidate is unselected. Writev remains build-gated with
no measured row and is unselected too. Neither exception enters 80f.

### Final latency rows

Both strict rows use all-gated r2 stamps and the exact selected 80f proxy.
All selected sends match, with complete authority audits and no cleanup
error. The uncapped row disables only the server limiter and retains the
same 650 players, layout and one-core bot limit.

| Final row | Overall P95 ms | Worst tile P95 ms | Proxy cores | Peak threads | Server STOP s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Strict D9 via 1 | 11.159291 | c, 11.612541 | 1.119717 | 1,330 | 3.365930 |
| Strict D9 via 2 | 5.720209 | a, 6.496125 | 1.045136 | 1,330 | 2.622106 |
| Uncapped, not D9 / not M4 | 4.709958 | a, 5.346459 | 1.030390 | 1,331 | 0 |

Via 2 per-tile P95 is a 6.496125, b 5.629792, c 5.033583 and
d 5.149459 ms. Uncapped per-tile P95 is a 5.346459, b 4.520291,
c 4.145333 and d 3.083917 ms. Bot STOP time is zero in both new rows.
Via 2 retains all 9,620 sends. Uncapped retains all 9,616. No tail was
trimmed; no unmatched packet or limiter pause was removed.

Via 2 has 395 of 482 P95-tail spans overlapping server STOP brackets,
81.95%, and zero bot overlap. This supports the limiter-dominated tail
verdict without claiming an exact causal subtraction. CPU bursts and
pause placement, not mean CPU alone, are the remaining lever across the
whole server group. Proxy-only tuning stopped by decision.

Latency verdict: M4 is not met under D9. Both strict final rows fail the
unchanged overall P95 line. The uncapped result is diagnostic only and
does not replace either failure. Tile a remains above 5 ms even uncapped;
the stamps do not separately measure decode CPU time.

Via 2 summary: `data/E25/e25-final80f-r1-via-2-summary.json`, SHA-256
`5dd2a8e3952f1ca72bb4ef9d693f5d1c99e784832f3fdef0dbd48dba4b59f755`.
Via 2 raw decompressed SHA-256:
`b91ae8c16fec292a27e190a24f5de5a7e17d3f0c1d04c4fbe4cdf4ad9a33f9dc`.
Uncapped summary: `data/E25/e25-final80f-r1-uncapped-summary.json`, SHA-256
`ffc82430902298ab89d05ccb5cae1df7f70a50199719b617574a63422cb64f14`.
Uncapped raw decompressed SHA-256:
`2897fbc354358c6ba55893676047a0be4cc3e0c869f51f37c58a981cba4a2d85`.
Final session: `data/E25/e25-final80f-r1-result.json`, SHA-256
`55d62f1bead4ae6a57c829fa186aea7cd74fcd87686b5e741b1d6cf51913c5b2`.
Overlap: `data/E25/e25-final80f-via2-stop-overlap.json`, SHA-256
`f2449e181c6fdddaf56f5bf6de7f585469bf243aad7bc42b083e9f45a0bba2fc`.

### Final evidence and cleanup

The complete published-evidence index is
`data/E25/e25-evidence-index-final-r1.json`, SHA-256
`4d209a12891231e7af8577db441e099845663b9db17f4988752aa322ac8ca71b`.
It lists all 194 artifacts with decompressed hashes for gzip files.
Naming, compression and private-path checks report zero violations.
All 48 resource-free tool contracts pass. No raw row was repaired or trimmed.

Plain `scripts/clean-builds` completed and freed 0 MiB. Root, Horizon,
Horizon-tiles and Meridian targets were all below their caps. E25 owns no
measurement window and has no queued load or build. Both owned source
checkouts were restored after their final builds. No commit or push was made.
