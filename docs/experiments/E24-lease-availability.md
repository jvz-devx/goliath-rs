# E24. Lease availability

## Status

The first step 1 attempt ended without reproducing an authority latch. The total window was
2,681.577 s, with 2,595.001 s of measured 650-player traffic. It reached the
cleanup reserve inside the 45-minute limit. This is not a full 45-minute
steady-state soak and does not explain the earlier failures.

A later frozen-proxy control, `native-650-r1`, reproduced an authority latch.
Tile c stopped for `action-clock-anomaly` with 780.479 ms still before its
keeper deadline. The retained clock readings prove why this invocation stopped.
They do not retrospectively establish the cause of E22 or E23. A separate
read-only four-clock probe later confirmed a fixed raw-pair offset across every
one of 5.94 billion triples.

The approved store and H13 clock fix are now frozen in
`.local/bin/goliath-e24/lease-fixed-r1/`. All 74 root tests and strict gates,
12 scoped Horizon clock/lease tests and the release gates passed. The enhanced
1,200-second probe admitted exactly one raw-pair offset over every triple.
The first fixed-stack soak failed at 1,671.088 s of retained measured traffic,
on a real keeper expiry after a transport timeout and post-call sleep. H13's
raw-clock guard did not fail. The approved H13 r2 and store-memory patch are now
frozen as `lease-fixed-r2`, with 82 root tests and 13 scoped Horizon tests
passing. Its cap is 9,216. Horizon was restored and returned to E25. The r2
30-minute soak passed availability, ownership, cargo and judged RSS criteria.
Receipt fill was only 31.889 s before minute 5 and is explicitly fragile.
Proxy RSS grew 16.45%, so M4's final-proxy soak remains open. The M3 rerun
completed all 1,000 crash seeds and all 18 partition/stale-login cases with
passing independent per-window and aggregate checks. The raw crash records
round-trip byte-for-byte through their retained compressed archive.
E25 owns proxy memory and latency work. E24 reports frozen proxy RSS without
judging it in this soak.
No commit, push, upstream operation or milestone completion is part of E24.

## Source findings before the run

The patch-06 renewal worker owns the `Client` used for its claim. The worker
moves that handle into its closure. Gameplay creates other handles through
`cluster::connection`. Shared gameplay traffic blocking that same connection
is therefore not established on this stack.

The control error `local cluster lease expired` does not identify the reason
authority stopped. The same error follows a clock anomaly, an expired keeper,
a refused renewal, a rejected late grant or failure of the post-observe check.
Step 1 records each branch separately before changing any lease rule.

Store receipts have no retention bound. Every applied mutation adds its full
command and response to `State::receipts`. Snapshots encode that entire map
on the serialized engine thread before handing immutable work to the background
worker. Background duration is not the duration of this serialized step.

## Diagnostic source and ownership

The reviewer assigned `.local/src/horizon` to E24. The other Horizon workspace
is untouched. The full dirty source, original Git index, porcelain status and
per-file hashes are backed up in `.local/evidence/E24/source-backup/`.
All 63 dirty source paths matched E22's patch-11 byte proof, with no extra dirty
paths. Reversing only patches 11 and 10 then reproduced FINAL-09 exactly through
a separate Git index. The real index is unchanged.

The overlay adds optional renewal, client-lock and clock-failure telemetry.
It does not change L=1,000 ms, margin=100 ms, renewal period=250 ms,
original-send anchoring, retry identity, keeper latching, store authorization
or the cut IO budget. Client traces belong only to the claim/renewal handle.
Other handles retain tracing disabled. Wall timestamps correlate evidence;
they never grant authority.

The trace records the original ticket time, actual call and wire phases,
reply time, granted counter, previous and installed keeper deadlines, and
the first stopped-authority cause. Clock failures retain both Instant brackets
and both continuous-clock samples. Every subsequent diagnostic record is
separate from the frozen E22 and E23 bundles.

Synchronous diagnostic writes can perturb scheduling. Results from this
instrumented binary are diagnostic evidence, not capacity qualification.

## Step 1 protocol

Use the retained reactor-1 proxy, FINAL-09 plus the diagnostic overlay,
the unchanged D9 layout, 650 packet clients, 256 pigs, a four-core server cap
and a separate one-core bot cap. The production server cap includes proxy,
store and sidecars. The limiter implementation is unchanged from E20.

The run owns a unique `.local/quiet-request`, starts only with one-minute load
at most four, and uses no acknowledgements after the explicit host handoff.
Outside processes must each average at most half a core. Their names are
redacted to `outside` in retained rows. The window is bounded to 45 minutes,
including startup and cleanup reserve. An authority latch ends it early.
Raw traces, environment and partial tick drains stay under `.local/evidence/`.
Large raw files are compressed losslessly. Existing frozen evidence is never
overwritten.

After step 1 the builder releases the host for the reviewer's E23 reproduction
and pauses all load and builds. Analysis and the fix proposal can continue.
No fix build starts without approval and a new host handoff.

## Step 1 result

The [small result](data/E24/diagnosis-r1-results.json) and
[causal readback](data/E24/diagnosis-r1-causal-readback.json) retain the arithmetic
and hashes. Raw observations stay under `.local/evidence/E24/diagnosis-r1/`.

All 40,652 renewal replies installed successfully. There were no authority-stop
records, clock anomalies or renewal IO errors. Maximum ticket-to-reply time
was 68.207667 ms. The smallest old-deadline slack when a reply was observed
was 464.154708 ms. The client-lock trace contains only directory, claim and
renewal calls. It contains no gameplay traffic and no unfinished call.
Call-waiting to lock-acquired time includes scheduler and logging delay;
it is not, by itself, time blocked behind another caller.

The recorded chain stayed healthy: original-send ticket, client lock,
request write, real grant reply, then deadline installation. No failing chain
was observed. Renewal starvation, tight deadlines and clock-gate failures
remain possible explanations for the older events, not findings from this run.
Instrumentation can alter scheduling, so this non-reproduction rules none of
those rare conditions out.

All 2,417 retained measured bot samples contained exactly 650 online clients,
with zero reported failures. D9's interior and crossing cohorts and 256 pigs
were unchanged. The server cap included all four tiles, proxy, store and four
sidecars. Start one-minute load was 2.471680. Maximum outside-process CPU was
0.062932 core over the whole fixture and 0.062948 over the measured span.

The original cell is labelled failed because the quiet-window timeout happened
before its normal `steady-last` endpoint. Its original noise flag is false
for that missing endpoint. The readback applies the same half-core rule to
the actual `failure-before-cleanup` boundary instead. Both complete spans pass,
and the boundary arithmetic has a test. The raw cell was not rewritten into a
capacity pass.

The store completed 169,522 batches. Maximum batch-start spacing was 290.747 ms,
execution 41.200208 ms, barrier 41.179417 ms and serialized maintenance
101.975167 ms. Background work reached 1,215.476042 ms while renewals continued
successfully. A background duration exceeding L is not a store-wide stall.

## Memory result

These are native per-process RSS readings. The comparison uses samples at
measured seconds 299.730 and 1,800.416, without extrapolation.

| Process | Minute 5 MiB | Minute 30 MiB | Growth |
| --- | ---: | ---: | ---: |
| Store | 76.969 | 204.125 | 165.21% |
| Proxy | 616.359 | 933.172 | 51.40% |
| Tile a | 438.219 | 449.422 | 2.56% |
| Tile b | 455.359 | 471.703 | 3.59% |
| Tile c | 459.953 | 472.234 | 2.67% |
| Tile d | 438.219 | 457.063 | 4.30% |
| Sidecar 0 | 2.813 | 2.922 | 3.89% |
| Sidecar 1 | 2.875 | 2.844 | -1.09% |
| Sidecar 2 | 2.813 | 2.750 | -2.22% |
| Sidecar 3 | 2.859 | 2.906 | 1.64% |

At the final measured sample, second 2,594.944, the store was 283.094 MiB,
267.80% above minute 5. The proxy was 1,064.125 MiB, 72.65% above minute 5.
Every tile and sidecar remained below 10% growth.

The receipt map explains an unbounded store allocation. Proxy RSS growth is
confirmed, but its allocation owner is not. A receipt-only change must not be
called sufficient for the requested per-process memory line. Further proxy
allocation counters or a heap profile need a separately named diagnostic
attempt after the host returns. No proxy source has been changed.

For that source investigation, the exact reactor-1 stack was reconstructed
in a private index and 37 reference files were hash-checked against its manifest.
The real Meridian index and checkout were untouched. The source is MIT licensed.
Its IO output queues have hard caps and clear on close; its optional path
histograms are fixed-size. Those facts do not attribute the RSS increase.

## Step 1 verification

The Rust architecture audit found no missing ownership module docs,
kind-named modules, wildcard imports or undocumented unsafe blocks in our
crates. Five exact clock-bracket boundary cases passed in the diagnostic
analyser. The current five resource-free tests cover bracket edges, a widened
scheduling bracket, call attribution, chunked log round-trip and failure-span
noise arithmetic. Python syntax checks passed for the E24 tools.

The first root strict Clippy run rejected a collapsible diagnostic branch and
a test module placed before production items. Both were corrected. That failed
attempt is retained separately. Root formatting, strict workspace/all-target
Clippy and all 53 tests then passed. Scoped Horizon strict release Clippy and
all four lease/clock tests passed. The foreground transport timed out after
five minutes and the driver lost stdout after those gates completed. Its
completed records are retained in `diagnostic-r2-build.json`. No second build
ran concurrently. Attempt r3 reused only matching successful gates from the
same byte-proved source and ran the missing release build once, in 237.786 s.
That build passed.

The diagnostic bundle is `.local/bin/goliath-e24/diagnostic-r3/`. Its engine is
`0d754193aa14e65a3bc360a1af814fee78769850becac4453c01022a37bfffb3`.
The frozen proxy, store, sidecars and bots are unchanged. The bundle has a
validated leased launcher contract and its own source proof.

The independent E23 raw-store readback found 94,547 batches, maximum start
spacing 283.030 ms, maximum execution 44.759208 ms, serialized maintenance
47.034 ms and background work 952.290833 ms. All 116,355 limiter signal events
were retained with no drops. These agree with the reviewer's measurements.
They do not establish the tile-side cause.

Context7 and the local macOS clock manual were checked for API interpretation.
The installed Rustix 1.1.5 clock-ID definitions were inspected after verifying
their MIT licence. No external implementation was copied. The documentation
distinguishes adjusted monotonic and raw clocks, but this run recorded no
clock rejection. That distinction is a lead only.

## Approved store implementation

The initial 4,096-entry proposal was too small for the requested margin.
The implemented FIFO holds 20,480 accepted mutating journal records. Reads,
refusals and receipt hits do not advance it. Snapshot iteration preserves FIFO
acceptance order rather than request-ID sort order, and recovery repeats the
same eviction order. Full commands are retained, so payload-conflict checking
has not been replaced by a weaker hash comparison.

Oversized legacy snapshots are refused before any file creation or journal-tail
repair. A test includes a torn journal and absent auxiliary files and verifies
that refusal changes no file bytes or file list. History migration is not
automatic. Old clients must be stopped or reconciled before upgrading.

The client records an overall deadline before waiting for its handle. Each
attempt and each IO phase checks that deadline. Lock waiting is bounded;
connect, whole-frame write and whole-frame read use the lesser of their phase
timeout and the remaining overall budget. No fresh attempt is sent after the
budget. Expiry returns a timeout saying the result is unknown, not aborted.
An already-sent command is not cancelled, and a manual later `call` starts a
new call budget. Those late messages are covered by the operation safety
argument, not an eternal promise of an accurate cached response.

### Retry budget and operating point

The cluster's longest configured Rust client uses three attempts and a
three-second timeout for each of three phases, including reconnects:

```text
T = 3 attempts * (3 s connect + 3 s write + 3 s read) = 27 s
required receipts = 1 original + 4 * ceil(T * peak mutation-rate upper bound)
```

The renewal handle's 200 ms timeout and one attempt have a smaller 600 ms
budget. The store's trace exposes all commands, not mutation tags. Counting
every read and refusal as a mutation is conservative. The calculation uses the
higher rolling one-second peak at batch start or execution end, then adds two
whole maximum-sized batches for endpoint uncertainty. It also checks the exact
rolling 27-second burst. No average-rate extrapolation is used.

| Observed workload | Start/end peak commands/s | Maximum batch | Conservative mutations/s | 27 s budget | Required at 4x | Margin at 20,480 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 650 players | 116 / 117 | 5 | 127 | 3,429 | 13,717 | 5.972x |
| 1,050 players, overloaded | 142 / 142 | 6 | 154 | 4,158 | 16,633 | 4.925x at this observed rate only |

The operating guarantee is the qualified 650-player, 20 TPS operating point,
with 5.97x measured-rate margin. The 1,050 trace ran at 5.42 to 5.70 TPS. Its
crossing and game-driven traffic does not bound a future 1,050-player cluster
at 20 TPS. A larger cluster must re-measure its peak rate at 20 TPS and size the
window from that rate. The 1,050 row is supplementary overloaded evidence,
not capacity acceptance or qualification at 20 TPS.

The calibration retained all 1,050 clients in every one of its 88 measured
samples, with valid outside noise and no observed lease latch. Its raw trace is
under `.local/evidence/E24/rate-1050-r1/`. The complete derivation and input
hashes are in [the operating-point budget](data/E24/retry-budget-operating-point-v3.json).
16,384 gave only 3.94x against the conservative overloaded calibration, so the
cap was raised to 20,480. Arbitrary future load or unbounded stalls are not
proved by a measured peak. Eviction safety instead comes from the next table.

### Every mutating operation after receipt eviction

This table concerns an unchanged command. Existing fence and lease checks still
run before retry lookup. It does not grant an expired holder authority merely
because an old success was cached.

| Operation | Result when its receipt is gone | Safety consequence |
| --- | --- | --- |
| `Claim` | Refused on a leased tile. Otherwise allocates a newer process fence and epoch. | Availability cost. The directory has one current fence and rejects the old one. |
| `ClaimLease` | `LeaseBusy` while the retained grant is live, even after release. After expiry it may allocate a new fence. | No early replacement. The old local keeper must stop before store expiry under the existing E16 clock and critical-IO assumptions. |
| `Create` | Existing integer player gives `AlreadyExists`; a stale owner fence is refused first. | No second player record or overwritten inventory. |
| `CreateUuid` | Same rule for the full UUID record. | No second record or overwritten inventory. |
| `Commit` | Its exact already-committed next-generation, target-owner and payload state returns the same generation without changing the record. Later progress or a different payload gives `GenerationConflict`; stale fences fail first. | Harmless idempotence or refusal, never a second generation advance. |
| `CommitUuid` | Same CAS kernel for the UUID record. | Harmless idempotence or refusal. |
| `Release` | A leased, already-released holder is refused. A matching legacy fence may set inactive again. A replaced fence is refused. | No reactivation and no shortening of the retained lease deadline. |
| `Heartbeat` | Legacy only. A live matching fence applies `max(stored, supplied)` again. Leased or stale holders are refused. | No counter rollback, grant or lease extension. |
| `RenewLease` | An accepted counter is now equal to or below the stored counter and fails as `RenewalReplay`, or the holder is stale, expired or draining. | No replay extension of a lease. |
| `BeginTakeover` | Repeats the drain on the live current lease, or is refused after replacement or expiry. | Idempotent drain; never grants a replacement or shortens the last grant. |
| `SetClock` | With the new guard, an older anchor or equal-anchor/different-body request gives `RequestConflict`. An identical whole clock at the same anchor may be stored again. | No rollback of a newer live clock anchor. Lease authority is unaffected. |

`Directory`, `Read`, `ReadUuid`, `Players`, `Clock` and `CheckLease` do not mutate
state. `Compact` is not a receipted state mutation; repetition compacts the
current durable state and does not repeat a business effect.

The CAS distinction above is important: the existing kernel recognizes an
exact already-committed outcome even without a receipt. It need not fail just
because the old request ID has been evicted. Tests cover both that harmless
success and rejection after the record advances again.

Thus a very late retry can lose its original answer, and claims can incur an
availability cost, but it cannot duplicate a stored player/item record or
produce two current authorized fences. Physical old-owner exclusion still
depends on the unchanged local lease gates and E16's clock and bounded-IO
assumptions. No stronger unbounded-pause guarantee is claimed.

### Clock guard and wall-time limitation

`unix_ms` orders clock-state updates only. It never grants or extends a lease.
Live writes require a strictly newer anchor, or an identical whole clock at
the same anchor. Cached replies remain no-ops. Historical journal replay keeps
previously accepted history, including an older write, rather than applying a
new admission rule retrospectively.

Because the anchor is wall time, a legitimate new update after a backwards
wall-clock step is refused until a new request carries an anchor beyond the
stored one. This is an intentional availability limit. A test proves the
refusal, later acceptance and unchanged monotonic lease deadline. The frozen
launcher writes `SetClock` only when no clock exists, so its initial write is
not blocked by the ordering guard. It reads an existing clock without writing
one. The frozen Horizon integration has no `SetClock` caller. No present live
update path was found that would hit this limit; future time/weather updates
must account for it.

### Store gates and frozen release

Root formatting and workspace/all-target Clippy with warnings denied passed.
`just test` passed all 73 tests. New tests cover the 27-second deadline,
no wire bytes after expiry, bounded handle waiting, the maximum calibrated
intervening-mutation budget, FIFO boundaries, every mutating operation after
eviction, snapshot order, journal recovery, both compaction paths, backwards
wall anchors and refusal of unsupported history before file repair.

The store release is
`.local/bin/goliath-store-e24-a7988a425/retention-r1/goliath-store`, SHA-256
`71e7840cf71fab15871d9b19ebf446bf65e7826a15b0c7468e4bca6023765fe5`.
Its [build record](data/E24/retention-r1-store-build.json) retains exact source,
gate hashes and the source archive. This is not yet fixed-stack soak or M3
qualification, and a count cap alone is not proof of the RSS line.

## Heap diagnostic preparation

Initial separately named variants used DHAT 0.3.3 with release symbols and
explicit live snapshots. Captures were dumped before teardown. Heap bytes and
RSS are different measurements.

The variant is built from frozen reactor-1 source and hash-verified original
internal dependencies. It does not silently include the new store/client
implementation. Meridian's current source and original index are backed up
and restored around the build. Frozen reactor-1 is untouched.

The instrumented binary compiled once in 62.510 s, SHA-256
`2134ef2c4d3188ba04e67781f0434a98b58c8efb4812222c48e3c167ee3d3845`.
Changed-file formatting and strict release Clippy passed. The full proxy test
run passed 562 tests and failed the unchanged listener-burst test after 128
queued sockets. This host's `kern.ipc.somaxconn` is 128, and the earlier release
report records the same limitation. The failed test is retained, not changed
or skipped. This remains a partially verified diagnostic, not a production or
capacity-qualified release. The unrelated frozen benchmark client's full-
workspace formatting failure is also retained. Later publication reuses only
the identical successful release artifact and matching verification evidence.

The early `heap-r6` bundle is retained but superseded. Its first runtime exposed
an instrumentation bug: the launcher's configuration-check command consumed
the one-shot output path. The next variant started only in the serving path.
Full-stack DHAT then proved too intrusive at 200 players: cold-join and
post-join captures hit the existing outstanding-ping limit. No lease latch was
recorded. The failed attempts, pre-teardown profiles and native warnings remain.
They are not successful load or memory qualifications.

The post-join DHAT capture lasted 4.213 s and retained 4.93 MB of newly tracked
allocations. Its profile is useful evidence about the disturbed attempt, not
an attribution of the original long-run RSS increase. No reply limit or timeout
was relaxed to keep the profiler running.

### Lower-overhead counter and sampler

The current diagnostic uses `goliath-alloc`, a separate application-internal
safety/profiling boundary. It is not a dependency of the model or store.
Meridian explicitly selects its allocator only in the diagnostic feature.
The crate forwards original pointers and layouts to `System`, counts requested
Rust bytes from process start, and takes sparse retained-stack samples only
after an explicit start. It adds no object header and owns no threads. The
unsafe forwarding code is isolated outside Meridian's unsafe-code prohibition.

The sampler has a bounded live-address map and suppresses recursive sampling.
Addresses are retired before System can recycle them. Reallocation failure
restores its old sample. Diagnostic failures disable sampling rather than
unwind through the allocator or change the System result. The explicit panic
boundary protects only this optional instrumentation. Global byte counts
include diagnostic bookkeeping, but not native malloc allocations or VM/stack
residency outside Rust's allocator.

A forwarding/reallocation/cross-thread-release unit test, recursive global-
allocator smoke, root formatting, strict Clippy and all 74 root tests passed.
The earlier store-only release record still has its 73-test source closure;
the additional test belongs to the new diagnostic-only crate.

The bundle `.local/bin/goliath-e24/heap-r9/` uses this counter/sampler and the
original frozen internal store/model code, not the new bounded store. Scoped
proxy formatting and strict Clippy passed. Its full proxy suite retains the
same one native backlog failure and 562 passing tests. Frozen reactor-1 and
the original Meridian source/index are unchanged after restoration.

The counter-based 200-player control completed with 200 sessions throughout,
20 TPS and no bot failures. Requested live Rust heap changed from
107,527,736 to 106,744,149 bytes while RSS changed from 144.328 to 149.328 MiB.
There were no sampler failures, missed TLS callbacks or dropped samples. This
short control shows instrumentation headroom, not a long-run root cause.
The separately named 650-player capture completed 900.890 s with 650 sessions,
no sampler failures, no missed TLS callbacks and no dropped samples. Requested
live Rust bytes fell from 460,715,799 to 438,865,316 while RSS rose from
577.734 to 611.938 MiB. Tile TPS was only 17.57 to 18.05, so this perturbed
attempt does not attribute the original 20-TPS RSS growth. Its native stack
resident increase of 19.4 MiB was not an increase in mapping count. The first,
minute-5 and final counts were 1,330, 1,331 and 1,330 regions.

The alternative tracking crate's public documentation was checked, but its
implementation is MPL-2.0 and was neither read nor used. The diagnostic
allocator implementation is original code using the standard library.

### Frozen proxy thread and native malloc control

`native-650-r1` used the unchanged frozen reactor-1 proxy, not an allocation
wrapper. The diagnostic engine and old store were unchanged from the first
E24 run. A separate read-only sampler recorded native thread IDs and names,
stack regions, stack resident bytes, RSS and `vmmap -summary` every minute.
It observed nine points spanning 480.644 s before the later clock latch ended
the intended 900-second measurement. Complete names and IDs remain under
`.local/evidence/E24/native-650-r1/threads/`.

The active thread count stayed between 1,326 and 1,328 and the stack-region
count between 1,326 and 1,329. Every point had 650 player supervisors, 650
terrain managers, four proxy IO threads and four proxy control threads. Wall
workers changed IDs, with nine to eleven active, without accumulation.
Stack resident bytes stayed at about 32.1 MiB after the first 32.2 MiB sample.
This bounded control provides no evidence of a growing active thread set.

Across these same points, RSS rose 45.594 MiB and default-malloc-zone resident
bytes rose about 45.6 MiB. Live allocated native malloc bytes fell 19.5 MiB
and the live allocation count fell 15,544. Dirty malloc bytes fell 13.2 MiB.
The resident-minus-dirty gap grew to 58.8 MiB, and reported dirty free-space
fragmentation grew 6.3 MiB. No malloc bytes were swapped. These readings
attribute the observed bounded-control RSS increase to allocator residency
and free space, not a growing native live allocation set or stack mapping set.
They are not a complete attribution of the original 30-minute result.

The fixture failed before its planned endpoint. It has valid outside-noise
readback through failure, at most 0.115 outside core per process, but no final
game-time sample or complete capacity qualification. Partial tick drains
include tile-b tick P99 51.348 ms. Do not present it as a successful 20-TPS
capacity row or a memory-gate pass. No allocator change has been built.

Small readbacks are [thread counts](data/E24/native-650-r1-threads.json) and
[native memory](data/E24/native-650-r1-native-memory.json). Exact frozen binary
hashes remain in the run summary and the identities table below.

### Proxy-memory handoff to E25

E25 now owns the Meridian checkout, proxy-memory fix and proxy latency work.
E24 has no build or mutation in progress in that checkout. Every diagnostic
overlay is retained as `patches/meridian-09-heap-diagnostics.patch` and revisions
2 through 5. All published E24 builds restored their backed-up Meridian source
and index. The frozen reactor-1 binary was never replaced. E24's next soak
uses that frozen proxy, reports its RSS separately and does not judge it.

Evidence boundaries for the next builder:

- The original 650-player diagnostic had proxy RSS growth of 51.4% from minute
  5 to 30, but no heap attribution. That observation is not a live-object leak
  diagnosis.
- `heap-r9` uses the original System allocator behind E24's counter/sampler.
  Its 200-player control held 20 TPS, with requested live Rust bytes falling
  783,587 while RSS grew 5.0 MiB in about 60 seconds.
- The 650-player counter run held every session for 900.890 s, but only
  17.57 to 18.05 TPS. Requested live Rust bytes fell 21,850,483 while RSS grew
  34.203 MiB. Allocation traffic was about 2.53 million allocations and
  1.88 GiB of requested bytes per second. It is a perturbed workload, not a
  qualified attribution of the original 20-TPS result.
- Its native stack resident bytes rose from 42.4 to 61.8 MiB while stack
  regions stayed about 1,330. Native malloc resident bytes rose from 511.2 to
  536.7 MiB while allocated bytes fell from 467.1 to 442.7 MiB and reported
  dirty free-space fragmentation rose from 44.2 to 62.9 MiB.
- The unwrapped `native-650-r1` control directly matches the observed 45.594
  MiB RSS rise to about 45.6 MiB malloc-zone residency growth. Allocated bytes
  fell 19.5 MiB, stack residency was flat and active threads did not grow.
  Its early clock latch and partial tick failure remain disqualifying. The
  native evidence supports residency/free space for that bounded observation,
  not every byte of the original 30-minute result.

Candidate fixes considered, none built by E24:

- Compare a different permissively licensed allocator, such as mimalloc, in a
  new bundle with the same work, limits and 20-TPS qualification. Its free-page
  purging policy is a testable residency hypothesis, not an assumed cure.
- Investigate an explicit native-malloc reclamation policy in a separate
  variant. Measure CPU, page-fault and latency cost as well as RSS. Do not
  modify the frozen control or mistake a virtual reservation for memory use.
- Reduce allocation churn at attributed source sites if a qualified profile
  identifies them. The perturbed retained-stack samples do not justify calling
  an individual terrain or crossing object a leak.
- A memory/admission cap can bound operational risk, but forced disconnection
  trades memory pressure for availability and does not itself fix residency.

The exact allocator source used for `heap-r9` is frozen separately in
`.local/evidence/E24/heap-r9-allocator-source.tar.gz`. The working
`goliath-alloc` source subsequently gained sharded counters to reduce hot-atomic
contention. That later source has not been built into a published bundle and
is not the source identity of `heap-r9`. No claim from the earlier gates applies
to that later source until it is checked. R5's patch and the allocator archive,
not the mutable working crate alone, are the reproduction inputs.

## Reproduced clock authority latch

The [causal readback](data/E24/native-650-r1-causal-readback.json) retains the
full event chain. Times below are relative to tile c's trace origin, not the
start of steady-state measurement.

The separate [event result](data/E24/native-650-r1-authority-event-v1.json)
corrects the inherited status-poll flag, which remained false when bot failure
preceded the next successful status poll. Raw `authority-stop` records take
precedence. The original run summary is frozen unchanged. The harness now
reads these records for its summary, with a regression test for this case.
The inherited availability note about missing renewal traces is likewise not
an assertion about the separately retained E24 engine telemetry.

| Event | Relative time | Observation |
| --- | --- | --- |
| Renewal 2,378 ticket | 622.464972875 s | Original request time |
| Renewal reply | 622.469301000 s | Granted counter 2,378, 4.328125 ms round trip |
| Grant installed | 622.469500042 s | Deadline 623.364972875 s |
| Previous clock read | 622.584292125 to 622.584292250 s | Continuous value 178,766.206201 s |
| Next clock read | 622.584292375 to 622.584292542 s | Continuous value 178,766.206222 s |
| Authority latch | 622.584493542 s | `action-clock-anomaly`, 780.479333 ms of lease slack |

The two continuous values differ by 21,000 ns. The Instant read brackets allow
only 125 to 417 ns, with the existing 1,000 ns slack. The advance exceeds the
upper bound plus slack by 19,583 ns. The checker returns failure and the action
gate permanently stops its keeper. The generic client-facing message still
says `local cluster lease expired` even though the lease deadline was not met.

The previous tile-c CONT was 330.598 ms before the latch. The next STOP was
4.038 ms after it. No recorded limiter STOP brackets the offending 417 ns
interval. The store's largest batch-start gap was 264.318 ms, and its maximum
batch execution was 16.722 ms. No other tile recorded a clock anomaly or
authority-stop record. Tile-c and crossing clients then disconnected. This
attempt ends failed, without retrying or suppressing the latch.

The current macOS source compares adjusted `CLOCK_MONOTONIC` against Rust
`Instant`. The installed `clock_gettime(3)` documentation distinguishes
`CLOCK_MONOTONIC_RAW`, unaffected by frequency or time adjustments, from
`CLOCK_UPTIME_RAW`, which stops during sleep. Apple's
[continuous-clock documentation](https://developer.apple.com/documentation/kernel/1646199-mach_continuous_time)
identifies `CLOCK_MONOTONIC_RAW` as the continuous-time equivalent. A timed/NTP
adjustment is a lead, not yet a proven attribution of the recorded 21 us jump.
Both normal and info/debug `timed` queries over the eight seconds around the
latch returned empty arrays without errors. No clock setting was changed.

The approved read-only `clock-pair-r1` probe brackets MONOTONIC, MONOTONIC_RAW
and UPTIME_RAW reads with actual Rust Instant readings. It retains every
comparison rejected by the existing arithmetic plus one periodic row per
second. A separate UPTIME_RAW bracket surrounds each MONOTONIC_RAW read, so
sampling preemption can be distinguished from a raw-clock difference. It
runs for 1,200 s in its own quiet window with no load or build alongside it.
The source and binary are separately frozen. No clock fix is built yet.

R1 completed 1,200.000002 s and 6,107,605,148 sample groups, with no rejected
consecutive comparison for any of its three native clocks. Its 1 Hz raw and
uptime read brackets admit the same exact offset from Instant throughout the
retained rows. Raw first-to-last rate uncertainty is about plus or minus
0.0000351 ppm. Adjusted MONOTONIC does not admit a fixed offset and ends about
5.998 ms lower relative to Instant than its first row, with a nonconstant rate.
No matching discrete 21 us event was captured. R1 cannot establish the
all-triple fixed-offset intersection or width percentiles because it did not
record those aggregates. Its [readback](data/E24/clock-pair-r1-readback.json)
and raw data remain frozen with that limitation, not promoted to condition 1.

```sh
python3 -B scripts/e24-run --name native-650-r1 --players 650 --seconds 900 \
  --bins .local/bin/goliath-e24/diagnostic-r3 --host-released
python3 -B scripts/e24-thread-sample --name native-650-r1 \
  --binary .local/bin/goliath-e24/diagnostic-r3/meridian
rustc --edition 2024 -O -D warnings scripts/e24-clock-probe.rs \
  -o .local/bin/goliath-e24/e24-clock-probe-r1
python3 -B scripts/e24-clock-window --name clock-pair-r1 --seconds 1200
```

The sampler runs concurrently with the named control, not after teardown.
These are retained, single-use names. A reproduction must choose fresh names
and must not replace an existing frozen binary or evidence directory.

### Clock fix proposal, approved subject to exhaustive probe

Change only the macOS suspend detector in a new small Horizon patch. Use
unadjusted MONOTONIC_RAW between two UPTIME_RAW reads. Keep the resulting
baseline interval for the whole claim, including all renewals. An adjusted
MONOTONIC reading is diagnostic input only and cannot revoke or grant authority.
Linux keeps its existing BOOTTIME path pending separate platform evidence.

The pinned rustix 1.1.5 exposes only four clock IDs on Apple targets, not these
two raw IDs. Do not transmute its enum or relax Horizon's unsafe-code ban.
Propose a macOS-only `nix` 0.31.3 dependency with default features disabled and
only `time` enabled. Its MIT-licensed safe `ClockId::from_raw` and
`clock_gettime` wrappers accept the named Darwin constants re-exported through
`nix::libc`. This keeps the patch and the caller safe without adding a new
internal crate or writing another platform FFI layer. Pin and retain the
resolved dependency identity in the new bundle. This dependency change is
part of the proposal, not an implemented change.
The cached 0.31.3 crate archive has SHA-256
`cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d`.
The new patch must include Cargo.toml and Cargo.lock changes and match this
registry checksum. Its declared licence is MIT.

For baseline reads `U0_before, C0, U0_after` and current reads
`Ui_before, Ci, Ui_after`, cumulative suspension is inside this interval:

```text
lower = (Ci - C0) - (Ui_after  - U0_before)
upper = (Ci - C0) - (Ui_before - U0_after)
```

Authorize only if the entire interval is within one fixed allowance `[-A, A]`.
Accepting merely overlapping intervals is insufficient, because a wide read
bracket could hide suspension. Use a baseline no wider than 500 ns, before the claim. If a
current bracket is inconclusive because of scheduling, immediately resample at
most three times, four triple attempts including the initial read, without
changing the baseline or increasing A. A proven
out-of-range offset, reversed/read-failed clock or exhausted resamples
closes authority. The keeper's independent deadline check still runs after
sampling. A delayed resample cannot resurrect an expired keeper.

The intended allowance is 1 us, conditional on the complete raw-pair probe
supporting that value. The first 169 one-second readbacks have exactly the same
feasible raw-to-Instant and uptime-to-Instant offset, with no observed awake
drift. This is measurement evidence, not a universal hardware jitter bound.
Unexpected future jitter fails closed. No tolerance grows with elapsed time,
and neither an action nor a successful renewal forgives an earlier offset.

Keep L=1,000 ms, margin=100 ms, renewal every 250 ms, original-send anchoring,
fences, renewal counters, takeover draining and full-L store recovery unchanged.
Under E16's same clock-rate assumptions, a permitted cumulative 1 us suspension
adds at most about 0.001001 ms to the latest local expiry, giving about
900.902 ms after send versus store expiry no earlier than 999.001 ms after
grant. The remaining headroom is about 98.099 ms. This does not remove E16's
existing requirement that a side effect and critical IO finish inside the
authorized budget, including any pause after the final check.

Injected-clock tests must prove all of these before the fix is qualified:

- Real suspension advances only the continuous raw clock and latches authority
  even when the keeper's uptime deadline still has room.
- Forward and backward adjusted-clock steps, including the retained 21 us
  example, do not latch when the raw pair is unchanged.
- Repeated individually small suspensions accumulate across actions and
  renewals, and latch once the single allowance is exceeded.
- Long awake runs do not consume a time-growing tolerance or change the anchor.
- Wide initial brackets cannot establish an unsafe baseline. Wide current
  brackets either resample within the fixed limit or close, never mask sleep.
- Reversal, read failure, arithmetic overflow and exhausted resampling close.
- A latch remains closed, and expiry, request-send anchoring, old-deadline and
  critical-budget checks retain their existing behavior.

Retain detector-cause telemetry in the new diagnostic bundle. After approval,
run scoped clock/lease tests and release gates, then the fixed-stack 30-minute
650-player crossing soak and M3 crash, partition and stale-login campaign.
No semantic clock fix has been edited or built at this proposal stage.

The reviewer approved this design and the dependency conditionally. The
running first probe lacks an all-triple offset intersection and a complete
triple-width histogram, so it cannot satisfy the strengthened build condition.
Do not infer those missing statistics from periodic rows. R1 remains frozen
and its source is unchanged.

The approved follow-up uses separately named `e24-clock-probe-enhanced.rs`,
`e24-clock-window-enhanced` and `e24-clock-analyse-enhanced`. It will run for
another 1,200 s after R1 ends, with no load or OS clock changes. It retains the
intersection over every triple, an exact width histogram, p50/p99/p99.99/max,
counts wider than 1 us and 2 us, the longest consecutive run wider than 2 us,
and the narrowest triple. It also retains a proposed 500 ns initial baseline
and counts whole-interval inconclusive reads against that fixed baseline.
Its histogram quantiles and counts are independently checked by the readback.

The build may start without another approval only if the complete enhanced
probe admits one fixed raw-pair offset across its entire window. Otherwise
stop and report. Do not widen A. The fixed tiles must expose counts per tile
for checks, inconclusive reads, resamples and closures by cause, plus the same
triple-width distribution during the loaded soak. Any resample-exhaustion
closure is a soak failure, just like deadline expiry. Add an injected
SIGSTOP-like pause between triple reads that resamples successfully without
latching, since both raw clocks advance during a process pause.

After scoped Horizon tests and the root gates pass, request a host quiet
window before load. The next soak is 30 minutes at 650 with crossings, zero
duplicates, owner violations, expiry or authority closures, and under 10%
minute-5-to-30 store and tile memory growth. Report proxy RSS separately as
allocator residency. Run the M3 crash, partition and stale-login campaign on
the same fixed tile/store identities, then freeze hashes for reproduction.

The fix is numbered `horizon-13`, after the diagnostic patch 12. E26 owns the
other Horizon checkout and starts at patch 14. E25 owns Meridian from patch 10.
The approved source is being prepared while the enhanced probe runs; no Cargo
command or semantic fix build starts until its completed readback passes.
The second source selection rechecked every original file, status, staged
identity, index flag and resolve-undo record. Index stat-cache bytes had
changed, so both byte images were retained. Logical index contents were equal.
Restoration will return the original backed-up source and exact index bytes.

Host scheduling now uses `.local/bin/host-window`. E24 holds the enhanced
probe window and releases it immediately on completion, including during its
subsequent build/test phase. Every Cargo command runs through
`host-window build E24 -- ...`. A measurement still owns its separate
`.local/quiet-request`, and takes the coordinator window before starting.
E24 takes a new window for the soak and each M3 measurement. Other builders
may build in the gaps; E24's soak and M3 measurements have priority.

### Completed enhanced probe and H13 publication

The [enhanced readback](data/E24/clock-pair-enhanced-r1-readback.json) covers
5,935,565,161 triples over 1,200.000000166 s. Every triple contributes to one
fixed-offset intersection, exactly `[0, 0]` ns. This is zero observed awake
raw-pair drift, not a universal OS jitter promise. The exact width histogram
was independently recomputed. P50/p99/p99.99/max widths are
41/42/167/1,198,875 ns. There were 55,244 triples wider than 1 us and 52,678
wider than 2 us. The longest consecutive wide or fixed-baseline-inconclusive
sequence was two. The 1 us allowance was not increased.

The implementation requires a baseline at most 500 ns wide before the claim,
with at most three baseline attempts and no later rebasing. Each check has
one initial triple and at most three immediate resamples. A proven bad offset
or invalid/reversed read closes authority, as does exhausted resampling.
The keeper deadline is checked after resampling. A bounded histogram exposes
its dropped-entry counter; qualification requires zero drops. Raw
`authority-stop`, `clock-anomaly` and periodic `clock-metrics` records remain
enabled by `GOLIATH_LEASE_TRACE_DIR`. `cluster-authority` exposes the same
clock counters under its exact `clock` field.

Two initial resolver-only attempts failed before compilation because full
offline metadata tried to fetch unused workspace packages, ALSA and socks.
The retained third attempt uses dependency-free metadata, preserves existing
locked dependency identities, and validates nix 0.31.3/checksum before all
locked release gates. No semantic threshold was changed in response.

The self-contained `patches/horizon-13-lease-raw-clock.patch` applies directly
to FINAL-09 and includes the earlier diagnostic overlay. Do not apply patch 12
again. Its SHA-256 is
`a37d2cf285243c8ef624c09502169f9b673c5e61187f1aaac61d3e39b4c9daae`.
The final patch proof is byte-equal to every compiled and gated source file.
Publication rebased the patch representation only, without recompiling or
changing executable bytes.

| Frozen item | SHA-256 |
| --- | --- |
| H13 engine | `abb3aecf8da2a5afdcb3998188e4d82a858ea6c33acc4e56fc332de206aa53ec` |
| Bounded store | `71e7840cf71fab15871d9b19ebf446bf65e7826a15b0c7468e4bca6023765fe5` |
| Reactor-1 proxy | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| FINAL.json | `59fc1bc6c677bc1b3ac20df3a6bf178d393c0e840bf7495869cf3f7f3230bf63` |
| Internal source archive | `800f951224aa2e1bd9163bdb522da2d1af5d22e1850872af86b52a08d24ab8ad` |
| Restored original Horizon index | `020d12c787932d58d659444ed9d50e98c2f182393757605b3d0312030a275b25` |

The [build record](data/E24/lease-fixed-r1-build.json) retains gate evidence,
compiled source hashes and the source-proof chain. The
[input archive record](data/E24/lease-fixed-r1-inputs.json) retains our crates.
The original checkout was restored with `scripts/e24-fixed-source restore`,
including every original file, dirty status and exact index bytes. E25
received the bundle hashes and handoff directly. To select the fixed stack
from the restored patch-11 state, reverse patches 11 and 10, then apply the
self-contained H13. E24 will ask E25 before any further checkout rebuild.

### Known limitation of the generic configuration guard

This experiment qualifies only L=1,000 ms and M=100 ms. On macOS the corrected
generic guard, including the single A=1,000 ns suspension allowance, is:

```text
M_ns * 1001 >= 2 * L_ns + 1000 * 1001 ns
```

The existing guard budgets clock-rate drift but not A. At L=500 ms and M=1 ms,
the old guard accepts the configuration, while the corrected inequality is
short by about 1 ns of margin at the narrow boundary. This is a known generic
configuration limitation, not an observed failure of the fixed fixture.
The qualified 1,000/100 configuration retains about 98.099 ms of real-time
headroom under the same E16 rate assumptions.

The reviewer initially deferred this to a follow-up
`patches/horizon-13-lease-margin-r2.patch` or a separate small margin-guard
patch after E25 returns the checkout. The approved cadence rebuild now folds
the guard into H13 r2. Its unit test must reject the old narrow
boundary and retain acceptance of 1,000/100. No other configuration is
qualified here. If another rebuild becomes necessary, include the guard then;
do not interrupt E25 or silently change the current measurement binaries.

## Failed fixed soak and approved cadence r2

`soak-fixed-r1` is failed, not a 30-minute pass. It retained 1,671.088 s of
measured traffic, 27 ownership audits with zero violations and all 65 named
cargo items after recovery. One checkpoint was refused after the lease latch,
so the recovery must not be described as an all-tile clean checkpoint. There
is no minute-30 memory endpoint. M3 has not run on these binaries.

The [partial clock readback](data/E24/soak-fixed-r1-partial-clocks-v2.json)
uses the original process baselines and the last metrics at or before the
first authority-stop record. H13's raw-clock guard held for about 32 million
loaded checks per tile, 128 million in total. There were zero clock anomalies,
offset closures, exhausted resamples or dropped width samples. Tile b's only
closure was `keeper-deadline`. The loaded p50/p99/p99.99/max triple widths,
in ns, were a 41/42/1,333/15,176,416; b 41/42/1,042/12,801,292;
c 41/83/1,750/15,197,917; d 41/42/1,083/15,094,375. Widths near 2 us occurred
and resampled successfully. That is a separate clock-guard result, not a
successful soak verdict or an explanation of E22/E23.

The [causal trace](data/E24/soak-fixed-r1-causal-readback.json) establishes this
tile-b sequence, relative to the original send of installed renewal 6,547:

| Relative time | Event |
| --- | --- |
| 0 ms | Renewal 6,547 sent; its confirmed keeper deadline is +900 ms. |
| +25.589 ms | Reply 6,547 read successfully. |
| +25.756 ms | Grant 6,547 installed. |
| +280.652 ms | Renewal 6,548 ticket sent. No other handle held its client lock; wait was about 50 us. |
| About +282 ms | Tile b and store receive STOP. |
| About +716 ms | Tile b receives CONT, about 1.295 ms before the store. |
| +716.315 ms | Call 6,548 returns `WouldBlock`, not a granted reply. About 183.685 ms remain before the confirmed deadline. |
| +716.940 ms | Renewal loop begins another unconditional 250 ms sleep. |
| +900 ms | Confirmed authority deadline. No renewal installed since 6,547. |
| +905.912 ms | An action check observes expiry and latches. |
| +967.934 ms | Renewal thread wakes, too late. It sends no 6,549 request. |

The measured call is 435.663 ms, not a slow successful installation. That
failed call plus two completion-relative sleeps, including the final sleep
longer than the remaining window, explains the full gap. The log write for
the authority-stop record occurs later than its captured `now_ns`; use the
captured observation time for the expiry boundary.

The store connection row with the same request ID records admission 0.006 ms,
wake 0.441958 ms, write 0.015042 ms and total 436.374542 ms. The uniquely
duration-compatible batch has one command, queue 0.007667 ms, execution
435.903625 ms and barrier 435.884375 ms. The component sum differs from the
connection total by only 250 ns. This is an inferred batch association, not
a directly recorded request-to-batch ID. Store receipt, barrier-start and
barrier-end absolute timestamps were not recorded. The barrier lies inside
the execution interval, with only 19.250 us of non-barrier execution.

The limiter brackets tile b for about 433.724 ms and the store for about
434.801 ms. Tile b logs its timeout about 27 us after its CONT call begins,
about 1.267 ms before the store's CONT call begins. These intervals clear
physical disk latency as the explanation for a 435 ms barrier. The batch has
one command, so no other mutation in that batch needs to explain the delay.
Snapshot work completed about 29 seconds earlier and this batch reports no
background completion or snapshot serialization. Scheduling inside the OS
and exact signal-delivery times remain unobserved.

The approved H13 r2 uses the original send time, not completion time, for its
250 ms cadence. The serial stream has no overlapping calls or catch-up bursts.
A caller-supplied absolute deadline caps all transport phases, mutex waiting,
reconnects and internal retries. The chosen fixed reserve is 50 ms, one
20-TPS tick for result processing and the final clock/keeper check. It does
not extend authority or promise to preempt SIGSTOP. After resumption, an
expired call returns result unknown and an expired keeper still latches.
The grant deadline remains original send plus L minus margin, and fences,
counters, IO authority gates and raw-clock baseline are unchanged.

The shared caller-driven timing policy lives in `goliath-model`, with injected
clock tests for the exact trace, a pause beyond expiry, four sends/s under
immediate failures and a slow success without added sleep. `goliath-sim`'s
existing partition model does not model RPC cadence; its existing authority
and ignored-deadline controls are rerun alongside the shared policy tests.

Non-durable renewal is not part of this fix. Its safety argument would require
durable claim/fence/L before any early grant, a full immutable L quarantine
after every restart, and original-send/pending-counter/latching checks at the
keeper. Counter rollback could then permit replay to delay takeover, but not
extend the old keeper from a late reply. Publishing a speculative claim or
shortening recovery would break that argument. It would also not bypass a
whole-store STOP, and a single worker could still wait behind an earlier
barrier. No such path or model change is built here.

## Store-memory proposal, approved and implemented

The [E25 control summary](data/E25/e25-memory-fixed-r1-control-summary.json)
records store RSS 65.219 to 137.313 MiB from minute 5 to 15, +110.5%, on
`71e7840c`. Our failed soak similarly records 77.922 MiB at minute 5, then
159.234 MiB near its end, with large cyclic peaks. The criterion remains
minute 5 to minute 30, never shifted to after warmup.

Any earlier three-minute fill estimate was an invalid inference from a peak
all-command upper bound. Reads and refusals do not fill receipts. Our store
compacts every 1,024 accepted journal changes. Only nine compactions have
completed by measured minute 5, versus 23 by minute 15. The average accepted
rate is about 25/s, and 20,480 fills near 13 minutes of store uptime, about
12 minutes of measured traffic. RSS alone cannot prove additional leakage
while this retained set is still filling.

The snapshot implementation nevertheless adds two large buffers. The main
path encodes the immutable snapshot into a Vec. Its background worker builds
another framed Vec of the same payload before writing. The observed plateau
bands near 159/203/248 MiB are consistent with that live set plus roughly one
or two 44 MiB buffers. Precise payload and allocator attribution is not yet
instrumented. Journal files have no persistent production Vec staging buffer;
batch and framing vectors are local and bounded, though freed pages may stay
resident. Background snapshot times grow to about 0.65 to 0.71 s and main
snapshot/rotation work reaches about 42 ms. Neither coincides with the failed
renewal's single-command barrier.

A snapshot landing at minute 30 but not minute 5 could fail the 10% line by
itself: 159 to 248 MiB is about 56%. Reducing receipt count alone does not
remove that transient.

Proposed separate store patch:

1. Reduce the FIFO cap to 9,216 only after approval. The
   [barrier-bearing budget](data/E24/mutation-window-proposal-v3.json) counts
   every command in each positive-barrier batch as an upper bound on accepted
   mutations, and counts whole boundary batches by execution-interval overlap.
   It adds a planning allowance of 20 renewals/s for the cadence change,
   without subtracting existing renewals. The resulting rate bounds are
   70/s at 650, 81/s in the overloaded 1,050 trace, and 72/s in this soak.
   Required capacities at 4x are 7,561/8,749/7,777. The proposed cap gives
   4.876x/4.214x/4.740x. These are measured-workload planning margins, not
   arbitrary-load guarantees. The overloaded trace remains about 5.5 TPS,
   never a 1,050-player/20-TPS qualification. Recheck the new cadence trace.
2. Reuse the encoded snapshot Vec between compactions, returning it from the
   completed worker to the engine instead of dropping it. One snapshot stays
   in flight. A 9,216-entry set is already reached by the ninth compaction,
   before minute 5 in the measured trace, so its buffer warms before that
   endpoint as well. No padded load or shifted measurement window is used.
3. Write the snapshot frame header and borrowed payload directly to the
   private next file, preserving the existing checksum bytes, full sync and
   atomic rename. Remove the second full-payload frame Vec. This is a memory
   representation change, not a relaxed durability barrier.
4. Expose accepted sequence/count, receipt count/body bytes and snapshot
   len/capacity/in-flight state in diagnostic records. Record per-request
   batch IDs and exact barrier spans so future chains need no duration-only
   inference. Compare RSS with those live sizes, and use native VM readback
   if allocator residency remains unexplained.

Tests must retain the current retry, boundary, recovery, unsupported-history
and compaction cases; add byte-identical streamed-frame output and reused
buffer tests. Snapshots above the new cap must still refuse startup without
file changes. No automatic history migration is proposed. All crates get the
root gates, followed by the unchanged 30-minute soak criteria and M3 on the
new frozen binaries. The reviewer approved these changes and the cap is now
9,216. Timestamp interval bounds include a 2 us printing-precision pad; the
70/81/72 bounds and planning margins are unchanged.

## Initial receipt proposal, superseded

This was the pre-approval proposal. The approved implementation and larger
calibrated limit above replace it. It was not an explanation for the earlier
availability events.

Retain the latest 4,096 mutating journal records' retry receipts in FIFO
acceptance order. Renewals count. Reads, rejected commands and receipt hits
do not advance the window. Persist the order in snapshots and reproduce the
same eviction order during journal replay. Request IDs are opaque identities,
not sortable ages. Use no wall-clock TTL or serialized Instant.

For an unchanged request still inside that window, preserve the original
response without applying another mutation, subject to the existing authority
checks before lookup. Outside the window, the original response is not
guaranteed. An uncertain result must not be treated as an abort. Fencing,
generation checks and renewal-counter checks remain mandatory after eviction.
No evicted renewal may extend a lease by replaying an old counter.

The limit is a mutation-count contract, not a time guarantee. 4,096 records
equal about 61 seconds of all requests at E23's observed average completed
request rate; only a subset mutate. Bursts can consume the window sooner.
The existing 256 KiB command-blob limit makes even the maximum-sized receipt
case finite. Actual memory and retry headroom still need measurement.

Snapshot ordering, exact eviction boundaries, shuffled IDs, uncertain retries,
recovery and compaction need tests. An upgrade requires stopped or reconciled
old clients. A larger legacy snapshot with no trustworthy FIFO order should
refuse startup explicitly, without changing its files. Any migration of that
history needs separate approval. New acceptance-window guarantees start with
requests accepted by the bounded implementation.

No lease length, margin, clock rule or renewal scheduling change is proposed
without evidence identifying its role in an authority latch.

## Step 1 cleanup and retained evidence

At the step 1 handoff all its fixture processes and packing workers were stopped
and its quiet request removed. The exact source, original index bytes and
original dirty status were restored in the owned Horizon checkout. The diagnostic source
overlay was removed from that checkout and remains recoverable from patch 12.
The other Horizon checkout was untouched.

Native stdout and anticheat files grew too large for the generic in-memory JSON
bundler. Their open file inodes were preserved through hard links and only their
storage paths changed. Writers kept the same file handles. All eight files,
11,117,859,136 raw bytes, were streamed into chunked JSON gzip archives and
verified byte for byte before the redundant raw links were removed. The files
were recoverable at that handoff. Final evidence curation does not promise
recovery of these full stdout and anticheat bodies. It retains their metadata
indexes, the failure excerpts and structured timing, authority, memory and
environment traces. No logger, anticheat rule, binary or workload setting was
changed.

Final plain `scripts/clean-builds` passed, freed zero MiB and reported the project
at 10,577 MiB. Root, Horizon and Meridian targets were 41, 2,016 and 683 MiB.
No extra target or checkout was created. Worlds and temporary run folders were
removed. At that step 1 handoff no fixed-stack soak or M3 rerun had happened.

## Frozen identities

The bundle is `.local/bin/goliath-e24/diagnostic-r3/`.

| Item | SHA-256 |
| --- | --- |
| Engine | `0d754193aa14e65a3bc360a1af814fee78769850becac4453c01022a37bfffb3` |
| Proxy | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| Store | `6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af` |
| Diagnostic Horizon patch | `43f702efcbb0395c90ebae486a0fcf376fc566803f4938c34a60a3dcf888a4d7` |
| FINAL.json | `dd13fac7b910187e29712945122ac20154ef3cbff6db659432a34da247dda31d` |
| Build record | `fbe7136166aa6a63ea916de20d41aa149dff160df2ff1409741a7cd7697e04a3` |
| Horizon source proof | `3eb46610a10b264d00f0eaac8756ec834935194ae0d0fafe150633927974b21f` |
| Internal source archive | `82cbf9d20b228b83e94ed62d293261de995c732f09dca894f1fce49988aaaaf0` |
| Causal readback | `df4201c1b298e202492741a321430493f4379736b181d32a13ef2a1d87018684` |
| Small result | `d004b2e5615969ea23eef07cd485159fd34c84f566c1ef17e17279d6de8d449e` |
| Native log archive index | `197ec7268cf63a9d756f8a31c5559cd91169ee87aaabff13d2c67740dcf48f5e` |
| Anticheat archive index | `2412518195ff0466e80fbc560e3aaf9ce084d8fbf9569e83a39a37f1cc8b1f52` |

The result records every raw diagnostic file hash. The two archive indexes
record each native file's raw and compressed hashes. Build inputs, including
the exact internal-crate source, are retained separately from mutable source.

## Commands

```sh
python3 -B scripts/e24-source select
python3 -B scripts/e24-build --name diagnostic-r3 --reuse-gates-from diagnostic-r2
python3 -B scripts/e24-freeze --build diagnostic-r3
python3 -B scripts/e24-run --name diagnosis-r1 \
  --bins .local/bin/goliath-e24/diagnostic-r3 --host-released
python3 -B scripts/e24-analyse --name diagnosis-r1
python3 -B scripts/e24-results --name diagnosis-r1
python3 -B scripts/e24-tests
python3 -B scripts/e24-source restore
scripts/clean-builds
```

Those are retained attempt commands. Source selection, publication, analysis
and archive names are single-use. The completed gate records explain the one
interrupted driver and the matching-gate reuse. A new runtime reproduction
uses the frozen bundle and a fresh name, only after an explicit host handback:

```sh
python3 -B scripts/e24-run --name REVIEWER_FRESH_NAME \
  --bins .local/bin/goliath-e24/diagnostic-r3 --host-released
python3 -B scripts/e24-analyse --name REVIEWER_FRESH_NAME
```

No source rebuild is needed for that reproduction. Preserve the original-send
lease rules and CPU caps. Do not overwrite the retained diagnostic attempts.

## R2 frozen build and reproduction

`.local/bin/goliath-e24/lease-fixed-r2/` combines H13 r2 with the separately
frozen memory store. Root formatting, strict workspace/all-target Clippy and
all 82 tests passed. Horizon formatting, strict release Clippy, all 13 scoped
clock/lease tests and the release build passed. Injected-clock tests use the
same sans-IO renewal policy as Horizon. The first root test attempt retained
one stale all-command rate assertion; the approved accepted-mutation budget
was then applied and all gates rerun, without changing a runtime threshold.

The store preserves exact snapshot bytes. Tests compare streamed output with
the old full-frame encoder and recover the old snapshot after interruption
between header and payload and between synced write and rename. A further
test proves the same snapshot allocation returns from the worker and is reused.

| Item | SHA-256 |
| --- | --- |
| H13 r2 engine | `53b0918027a700d9c2684936beb218c922b229401b7a3344805d65831d0adb9f` |
| Memory store | `9636fa6d8089b071be241437e79eb2942a07dc756039b394e73cc8f705e26d0e` |
| Frozen reactor-1 proxy | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| H13 r2 patch | `f2c03265ceced9a3e8096bc7ee52f2d0da5b41f6c7fd79f9f845125b4cae389b` |
| Store-memory patch | `0d15f60e1ed0ca85af2bbdcb46f539226ecda7de0ac8105f55fb196b70a62352` |
| FINAL.json | `4914d701106683002a178cfbbf45248de2e95f44f6f269766853123e31dd5af0` |
| R2 internal source archive | `1d7a6e641e1b2def4583abc7acffd3123eb8f3ec51ed02a9d4ad28380fee9df0` |

The [combined build record](data/E24/lease-fixed-r2-build.json) and
[separate store record](data/E24/memory-r1-store-build.json) retain commands,
gate logs and source identities. The stack is FINAL-09 plus only
`patches/horizon-13-lease-renewal-r2.patch`, with our exact crates from the
input archive. Neither patch 12 nor H13 r1 is applied separately. E25 received
the restored checkout and exact hashes directly. The completed soak and M3
results are recorded below, separately from these build gates.

The retained soak command was:

```sh
.local/bin/host-window quiet E24 "r2 30-minute650-player soak"
python3 -B scripts/e24-run --name soak-fixed-r2 --players 650 --seconds 1800 \
  --bins .local/bin/goliath-e24/lease-fixed-r2 --clock-telemetry --soak-audit \
  --planning-mutation-rate 72 --host-released
.local/bin/host-window release E24
python3 -B scripts/e24-analyse --name soak-fixed-r2
python3 -B scripts/e24-clock-soak --name soak-fixed-r2
python3 -B scripts/e24-store-soak --name soak-fixed-r2 --planning-rate 72
python3 -B scripts/e24-soak-result --name soak-fixed-r2
```

Use a fresh attempt name for independent reproduction. The rate monitor stops
on a planning-bound exceedance and never adjusts the cap. The store readback
reports actual fill lead before minute 5; less than 60 s is explicitly fragile,
even if the RSS line passes. Every production process except the explicitly
unjudged proxy retains the under-10% minute-5-to-30 criterion. Helpers remain
reported separately. Snapshot capacity and in-flight diagnostics cover the
minute-30 transient. A new window is taken for each bounded M3 campaign segment.

## R2 soak result

The [recomputed result](data/E24/soak-fixed-r2-result.json),
[loaded clocks](data/E24/soak-fixed-r2-loaded-clocks.json) and
[store diagnostics](data/E24/soak-fixed-r2-store-diagnostics.json) pass the
agreed E24 lines. All four tiles held about 20 TPS for 30 minutes. All 29
ownership audits had zero violations or generation regression. Every minute
had crossings, 582 to 589 activations, and all 65 unique cargo items survived
the clean all-tile checkpoint and recovery. There were zero authority-stop
records, clock anomalies or closure counters, including during recovery.
Maximum outside-process CPU was 0.0808 core and executable identities were
unchanged.

| Process | Minute 5 MiB | Minute 30 MiB | Growth | E24 judgment |
| --- | ---: | ---: | ---: | --- |
| Store | 52.328 | 53.828 | 2.87% | Pass |
| Tile a | 453.406 | 461.703 | 1.83% | Pass |
| Tile b | 455.109 | 469.344 | 3.13% | Pass |
| Tile c | 435.500 | 457.031 | 4.94% | Pass |
| Tile d | 429.719 | 461.797 | 7.46% | Pass |
| Sidecars | 2.844 to 2.922 | 2.828 to 2.953 | At most 1.08% | Pass |
| Proxy | 575.703 | 670.406 | 16.45% | Reported, not judged here |

The proxy growth fails the final M4 soak line. That milestone stays open
until the final proxy passes its own soak; E25 owns that work. E24's result
does not tick the M4 box or turn a diagnostic into a proxy qualification.

Receipt fill is fragile. The window first filled at measured second
267.990, only 31.889 s before the actual minute-5 sample at second 299.904.
That is less than the agreed 60 s cushion. The pass still counts, but its
fill lead is small and must be reported for reproduction. No endpoint or cap
was moved. Accepted-mutation interval upper was 32/s, below the 72/s planning
bound. The 27 s budget is 864 and the observed retention margin is 10.6655x.
The 9,216-entry window stayed bounded. All 47 snapshots completed with at most
one in flight, and snapshot capacity was exactly 33,554,432 bytes throughout
the interval after minute 5. There was no second frame-sized snapshot buffer.

H13 r2 completed about 35 million loaded checks per tile, 140 million total.
There were zero dropped width samples or baseline changes. P50/p99/p99.99/max
triple widths, in ns, were a 41/42/1,042/12,977,208;
b 41/83/1,750/15,974,083; c 41/42/1,042/13,301,500;
d 41/42/1,042/15,521,125. Inconclusive reads successfully resampled
3,562/4,962/3,588/3,600 times. Widths near 2 us occurred under load, and no
allowance was widened. The clock policy remains A=1 us, a pre-claim baseline
at most 500 ns wide and at most three resamples.

## R2 M3 measurements

The original E13 crash campaign completed all 1,000 seeds, 240001 through
241000, on these same frozen hashes. Window 1 admitted 575 trials before its
1,200-second admission budget expired. It drained every in-flight trial and
saved a clean pause. Window 2 resumed the unchanged schedule, ran the remaining
425 trials and completed. No failed seed was rerun. The independent checker
recomputed named-item locations for every seed and verified all 15
phase/victim combinations, each represented 66 or 67 times. There were zero
duplicates, missing items or stale files. The 133 write-phase inventory takes
that rolled back to the source chest follow the original recovery accounting.

The separate matrix completed all 12 partition and six stale-login cases.
Every stale side effect was refused while the old process was still alive.
The independent checker verified lease fencing, unchanged player generation
and owner, and the original CAS refusal. Refusal by a dead process would not
pass this check.

The three measurement windows lasted 1,207.920, 893.740 and 106.565 seconds.
All passed the outside-process rule and binary-identity checks. Maximum
outside CPU was respectively 0.2923, 0.3155 and 0.0843 core. All fixture run
directories were removed, and E24 released its final measurement window.
These are builder results; independent reviewer reproduction is still required.
The [final independent result](data/E24/m3-fixed-r2-results.json) recomputes
every trial, verifies coverage, all 18 matrix refusals and all three window
records. Packing retained all 1,000 original JSON byte sequences and their
individual hashes in `campaign-trials.jsonl.gz`. The checker ran again through
the packed records and returned the identical result. The archive is
recoverable byte-for-byte. This guarantee covers the small crash-trial
records, not the disposable full stdout and anticheat bodies.

The qualified configuration remains L=1,000 ms, margin=100 ms and a
250 ms send-anchored renewal cadence. Each call, including its internal
retries, is bounded by the last confirmed keeper deadline minus 50 ms.
The original-send deadline, fences and counters are unchanged. The corrected
macOS guard is `M_ns * 1001 >= 2 * L_ns + 1000 * 1001`. Non-durable renewal
grants were evaluated only and were not implemented.

The store uses 9,216 FIFO receipts and reuses the snapshot Vec. Its direct
header/payload write preserves exact frame bytes, full-sync order and rename
order. The soak supplies the memory result, while the crash and matrix runs
supply the renewed safety evidence. Neither establishes the cause of the
earlier E22 or E23 events. Those causes remain unproven.

## Final evidence curation

The approved prune removed 65 uncited full stdout and anticheat payload
archives, 6,003,582,396 bytes. The exact path, size and hash of each removed
file remain in `.local/evidence/E24/final-prune-r1.json`, SHA-256
`a90dd3deee0d01b380f2b559f66ca97bfcc4f8a9b83c6ab8761e0a4c91c562c4`.
Its completion record is `final-prune-r1-completed.json`, SHA-256
`2264effd732979f93c297bfc322631c73446827891c317cd84e1579c0b7eb695`.
The removed bodies are not recoverable. Their metadata indexes remain as
historical identities, not as promises that the payload still exists.

Structured lease, client, store, limiter, process, memory and environment
traces remain, as do failure excerpts, frozen probes, gate logs, source/index
proofs, exact source archives and every frozen executable bundle. The crash
trial archive and all matrix records remain. No other builder's files were
selected. Plan generation and application both ran through the shared host
build gate, outside measurement windows. Plain `scripts/clean-builds` then
passed, freed zero MiB and left all four target directories under their caps.
The shared project measured 15,487 MiB at that check. E24's retained evidence
measured 528,008 KiB, 515.6 MiB, after pruning. The cleanup log is
`.local/evidence/E24/final-clean-builds-r1.log`, SHA-256
`a98eb92438d4108d73296fb456bc5b419a0ac3451815e5dfc7c074619ad8f398`.
The small [curation record](data/E24/evidence-curation-r1.json) records these
counts and identities. No E24 quiet request, measurement, build or packing
worker remains. Other builders' windows and checkouts are untouched.

## Final frozen hashes and reproduction commands

The selected bundle is `.local/bin/goliath-e24/lease-fixed-r2/`. These identities
were unchanged through the soak, crash campaign and refusal matrix. The build
records above retain the exact source inputs and successful gates. No build or
checkout change is needed to rerun the experiments.

| Item | SHA-256 |
| --- | --- |
| Engine | `53b0918027a700d9c2684936beb218c922b229401b7a3344805d65831d0adb9f` |
| Store | `9636fa6d8089b071be241437e79eb2942a07dc756039b394e73cc8f705e26d0e` |
| Reactor-1 proxy | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| H13 r2 patch | `f2c03265ceced9a3e8096bc7ee52f2d0da5b41f6c7fd79f9f845125b4cae389b` |
| Store-memory patch | `0d15f60e1ed0ca85af2bbdcb46f539226ecda7de0ac8105f55fb196b70a62352` |
| FINAL.json | `4914d701106683002a178cfbbf45248de2e95f44f6f269766853123e31dd5af0` |
| Exact internal source archive | `1d7a6e641e1b2def4583abc7acffd3123eb8f3ec51ed02a9d4ad28380fee9df0` |
| Soak result | `786ef62defccc20cd921c5e6093cba7929ae244f3cad47a78d26f9040f084ede` |
| Loaded-clock readback | `385dd19aba46a0de61020b06ab0fc6a5358c98932eb41b6b4c211e2f9a278bba` |
| Store readback | `8ea5462516256a55dc9d433a0e0fa559863ae2e88517f41377cdd8443a3013c7` |
| M3 independent result | `c3f06db9cb35cc6a55f82d9703afd8c49385e2a051848f55783a676e7cbb5c69` |
| Byte-identical crash-trial archive | `c8042223c5868967d929902c941fc7a07e6ed135f8494c21a78e07dc6b17abd1` |

The exact executed M3 commands were:

```sh
python3 -B scripts/e24-m3 campaign \
  --name m3-crash-w1 --data-name m3-crash-fixed-r2 \
  --window-name m3-crash-fixed-r2-w1 \
  --bins .local/bin/goliath-e24/lease-fixed-r2 \
  --seed 240001 --crossings 1000 --jobs 4 --budget-seconds 1200
python3 -B scripts/e24-m3 campaign \
  --name m3-crash-w2 --data-name m3-crash-fixed-r2 \
  --window-name m3-crash-fixed-r2-w2 \
  --bins .local/bin/goliath-e24/lease-fixed-r2 \
  --seed 240001 --crossings 1000 --jobs 4 --budget-seconds 1200 \
  --resume-from m3-crash-w1
python3 -B scripts/e24-m3 matrix \
  --name m3-matrix --data-name m3-matrix-fixed-r2 \
  --window-name m3-matrix-fixed-r2 \
  --bins .local/bin/goliath-e24/lease-fixed-r2 --budget-seconds 1200
.local/bin/host-window build E24 -- python3 -B scripts/e24-m3-results \
  --campaign-data m3-crash-fixed-r2 --campaign m3-crash-w2 \
  --matrix-data m3-matrix-fixed-r2 --matrix m3-matrix \
  --name m3-fixed-r2-results --pack
```

Names are single-use. For an independent runtime reproduction, substitute a
fresh common prefix in every run, data and window name, preserving the seed,
1,000-trial schedule, jobs, frozen binaries and admission budget. Each driver
queues its own quiet window and releases it after draining in-flight work.
Resume only a clean `paused-window`, never an actual failed seed. If a fresh
first window completes all 1,000 trials, omit the second window and use the
first campaign name for result checking. If another clean pause is necessary,
use a new window name and the immediately preceding campaign for `--resume-from`.

To independently recompute the retained evidence without launching a cluster
or packing again, use a fresh output name and omit `--pack`:

```sh
.local/bin/host-window build E24 -- python3 -B scripts/e24-m3-results \
  --campaign-data m3-crash-fixed-r2 --campaign m3-crash-w2 \
  --matrix-data m3-matrix-fixed-r2 --matrix m3-matrix \
  --name REVIEWER_FRESH_READBACK
```
