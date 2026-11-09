# E26. Direct tick costs

Status: the full diagnostic campaign is complete. Its combined under-5%
verdict is unproven because instrumentation is intrusive. The separate lite
CPU campaign fails the 5% line on warming alone after the declared charges.
The reviewer verified the failure by recomputation and a corroborating
fresh runtime row. The reviewer confirmed patch 10's warming reduction.
Complete final untimed and lite bundles are frozen; final-stack runtime
numbers are reserved for the reviewer's reproduction. The first
setup attempt stopped during joining and contributes no observation.
The reviewer must reproduce any result before accepting it.
No commit, push or upstream action is part of this task.

## Plan

Use FINAL-09, engine `b2f1ea65`, with the unchanged reactor-1 proxy and D9
fixture. E24's soak and M3 work have priority. Every build goes through
`host-window build E26`. Every load cell holds `host-window quiet E26` and
releases it after cleanup. Analysis never holds the window.

The original owned checkout is archived under `.local/evidence/E26/`.
The final stack applies in a private Git index at `fc2868b2b`. Its 60 changed
files now match the owned checkout byte for byte. The real index is unchanged.
The frozen executable matches its complete recorded SHA-256. The original
internal dependencies also reproduce from their recorded repository revision.
Their source archive is private. E24's current dependency edits stay untouched.
This is a source/provenance proof, not a claim of a bit-identical rebuild.

Patches 10 and 11 are inspected but are not selected in FINAL-09. They are
separate optimisation candidates. The diagnostic starts at patch 14 because
E24 owns patch 13.

## Timer coverage

The diagnostic uses monotonic timers on the dedicated main thread only.
Nested timers are exclusive. Their sum cannot count a child twice.
Each tick retains elapsed nanoseconds, invocation counts and the complete
category sum. The busy denominator includes the simulation, main-thread
control work between ticks, and chunk pumping. Explicit between-tick sleeps
and worker-completion waits inside pumping are excluded. Each row records
the excluded pacing wait separately. Limiter pauses within work remain in
elapsed time. The raw capacity ticks are unchanged.

| Added work | Direct coverage |
| --- | --- |
| Ownership and borders | Area lookup, containment, chunk ownership, body hold, write gate, unknown-read wall branches, piston/entity containment and generation filters |
| Leases | Core action gate, local keeper and clock checks, cluster require and fence access; off-thread renewals excluded from tick totals |
| Cut state | Container revisions, generation revisions and snapshots, cut-active save hooks, resident/entity capture, bootstrap, enqueue and completion drain |
| Control and prewarm | Added control timing and dispatch, cluster registration, full warm-generation call, approach/import/export/activation paths and live state |
| Readers | Reader predicates and radius selection, reader loading areas, reconciliation and observer-only chunk/entity delivery; real-player work stays separate |
| Status and digest | Added session status fields, population census, raw-tick recording, digest request and hook; report whether a region is armed |
| Pacing comparison | Remaining upstream wait-loop and chunk-pump work after exclusive added-work timers, with sleep excluded |

The coverage ledger will name each modified function or inline block and its
bucket. Startup-only and off-thread hooks are listed separately. A reachable
uncovered path prevents an under-5% claim. If a mixed routine cannot be split
safely, its full elapsed cost is a labelled upper bound, never an isolated
added-work estimate.

The timer recorder is bounded and reports dropped rows. Tests use a supplied
clock to check nesting, sleep exclusion, reset, thread separation and complete
category sums without relying on host timing.

The complete mechanical ledger is `data/E26/e26-hunk-coverage.md`. It has one
row for each of 311 diff hunks, with detailed scope records in the adjacent
JSON. A ledger row does not itself prove low runtime overhead.

A separately named positive-control executable adds one compiled-in 200 us
busy-wait per dedicated tick in the authority prelude. Run one 100-player
cell with 30 seconds of warmup and 15 measured seconds. Require one recorded
pulse per complete tick and a median pulse cost of 200 to 220 us. Compare
the actual additions fraction with the same row after removing the known
pulse from both numerator and denominator. This control is not capacity data.

## Fixed A/B method

At 650 players, use 30 seconds of warmup and 60 seconds of measurement. Keep
the four-core aggregate server limit, separate one-core bot limit, all 256
pigs, view distance, routes and reader behaviour unchanged.

A is unchanged FINAL-09. B is the separately frozen diagnostic. Use the fixed
order A/B/B/A, then B/A/A/B. Keep every attempt. No replacement run is selected
after seeing a result. Each cell chooses the busiest tile from native process
CPU deltas in the last five warmup seconds, as E23 did. Enable detailed timers
on that tile only after selection. Record all tiles' raw tick P50/P99 and CPU,
plus native main, cut-writer and renewal thread CPU on the selected tile.

Compare all four A and B rows. Report paired changes and within-arm spread.
The timer cost stays in the elapsed upper bound. Do not subtract a guessed
clock cost. If CPU or raw-tick shifts exceed measured noise, the diagnostic
is perturbed and cannot certify the strict under-5% line without a defensible
bound. The pass line uses the sum of all added main-thread categories divided
by tick busy time, strictly below 5%.

Select optimisation targets only after the profile. Each candidate gets its
own small patch and a same-session unchanged FINAL-09 control. D10 needs two
passes at a step. A closed bracket needs two failures at a higher step. Mixed
steps remain unresolved. Changes within noise are not wins.

## Evidence and verdicts

| Item | Verdict |
| --- | --- |
| FINAL-09 patch/source proof | Passed, 60 files and unchanged real index |
| Diagnostic source proof | Passed, 62 files after ordered patches 14 through 18 |
| Deterministic timer accounting | Four Rust tests passed on the earlier draft; current release gates are pending |
| Adapter and result arithmetic | Six Python checks passed without a load |
| Planted-cost control | Passed at 100 players, 305 complete ticks |
| Binary rebuild identity | Not claimed |
| Full-bundle additions share | Unproven for the original engine; instrumented upper envelopes are 28.35 to 31.94% |
| Timer overhead A/B | Intrusive beyond observed main-CPU and raw-tick spread |
| Lite warming lower bound | Failed at 5.2359% after charges; reviewer recomputation and fresh in-spread runtime row corroborate |
| Off-thread CPU | Measured separately, below |
| Clock-fix overlap | The clock patch changes the timed lease-check path; FINAL-09 numbers do not cover those bytes |
| Qualified optimisation | Patch 10 warming reduction confirmed by the reviewer; final-stack runtime pending |

## Full-bundle A/B result

The eight observations use the fixed A/B/B/A, B/A/A/B order. All pass host
noise, frozen-identity, adapter and shared-harness checks. Every tile retains
64 pigs and 65 readers. All 650 clients stay online, with no failed or late
rounds. No raw tick or timer row drops. Digest hashing stays unarmed.

The selected tile follows the last-five-warmup-seconds rule. It is not
reselected after measurement. Quantiles below are that tile's raw E11 clock.

| Row | Tile | Main cores | Raw P50 ms | Raw P99 ms | Tick gates | Instrumented additions |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| A1 | a | 0.32520 | 16.176 | 40.646 | pass | not timed |
| B1 | a | 0.37831 | 19.522 | 48.054 | pass | 31.200% |
| B2 | a | 0.40851 | 19.396 | 55.787 | fail | 30.019% |
| A2 | d | 0.31310 | 15.341 | 36.444 | pass | not timed |
| B3 | d | 0.39737 | 18.966 | 50.209 | fail | 31.936% |
| A3 | d | 0.33731 | 17.466 | 45.092 | pass | not timed |
| A4 | d | 0.32910 | 16.200 | 42.889 | pass | not timed |
| B4 | a | 0.41479 | 19.748 | 50.947 | fail | 28.354% |

Within-arm spread is the larger observed maximum-minus-minimum range of
the two arms. It is descriptive noise, not a confidence interval from four
observations. Every same-tile paired main-CPU change is positive:
0.05311, 0.08273, 0.06006 and 0.08601 core.

| Metric | A mean | B mean | B minus A | Within-arm spread | Inside spread |
| --- | ---: | ---: | ---: | ---: | --- |
| Selected main cores | 0.32618 | 0.39975 | 0.07357 | 0.03649 | no |
| Selected process cores | 0.57211 | 0.63726 | 0.06515 | 0.03628 | no |
| Raw P50 ms | 16.296 | 19.408 | 3.112 | 2.125 | no |
| Raw P99 ms | 41.268 | 51.249 | 9.982 | 8.648 | no |
| All-server cores | 3.36731 | 3.48490 | 0.11758 | 0.17535 | yes |

Selected main CPU rises 22.6%. Three diagnostic rows fail the raw tick-tail
gate, while all four unchanged controls pass. Total-server variation must
not hide this selected-thread perturbation. The combined strict line is
therefore unproven. The 30.3325% ratio of instrumented additions sums to busy
sums is an upper envelope in this bundle, not the original engine's fraction.
No guessed clock cost is subtracted to turn it into a pass or a failure.

## Pacing work and excluded wait

The full bundle retains 4,828 complete selected-tile cycles. Their busy sum
is 116.1583 s and their conservative additions sum is 35.2337 s. The warm
category is exclusive. It totals 0.9274 ms per cycle over 17.57 to 19.29
warm calls per cycle, not one inclusive warm call per cycle. Remaining
upstream pacing/pump work averages 0.00504 ms per cycle.

This separates the code paths behind E23's mixed 7.6% Running category.
Speculative warming is real work on our side. Upstream idle pacing is not
an explanation for all of it. The historical CPU-sample percentage cannot
be transferred numerically to this busy-wall denominator.

| Row | Busy P50 / P99 ms | Pacing sleep P50 / P99 ms | Worker wait P50 / P99 / max ms |
| --- | --- | --- | --- |
| B1 | 21.473 / 55.709 | 28.170 / 40.691 | 0 / 0 / 0 |
| B2 | 21.111 / 60.849 | 28.656 / 33.533 | 0 / 0 / 0 |
| B3 | 20.516 / 58.546 | 29.278 / 33.170 | 0 / 0 / 0 |
| B4 | 21.253 / 53.802 | 28.371 / 32.837 | 0 / 0 / 0 |

No worker-completion wait is observed in these four steady windows. Ordinary
pacing sleep is outside each preceding raw E11 tick interval and assigned
to the following timer row. Its contribution inside the raw P99 clock is
zero by source placement. It still occupies the full tick-to-tick cadence.
No wait or limiter pause is removed from any raw tick. A future nonzero
worker wait can follow reader or preparation demand; its cause is not
identified by these counters and must remain a separate quantity.

| Off-main owner, selected tile | A mean cores | B mean cores |
| --- | ---: | ---: |
| Cut writer, including speculative encoding | 0.046991 | 0.044756 |
| Lease renewal | 0.000116 | 0.000122 |

These are native thread CPU counters, not percentages of a tick. They are
not added to the main-thread timing fraction.

## Retained-data lower-bound attempt

`data/E26/e26-lower-bound-1.json` tries the requested read-only proof.
Inclusive warming and child-timer invocation context are absent from the
full bundle. The 200 us pulse validates a known span, not a physical upper
bound for every timer. Its observed 0 to 7 us excess also contains busy-wait
overshoot and scheduling. Deterministic tests use supplied clock values.

The A rows retain raw E11 durations, which omit the control prelude between
ticks. Main CPU also cannot upper-bound busy wall time that may block. The
safe mean busy upper bound from the query brackets is the whole cadence,
at most 50.0553 ms per cycle. The exclusive warming numerator gives at most
about 2.07% against that conservative denominator before overhead correction.
The few-timer enqueue and unarmed digest cases also do not establish 5%.
Only a trivial corrected lower bound zero is supported. That is a limit of
the proof, not an estimate that warming costs zero. The verdict stays unproven.

The reviewer approved a new lite experiment using exactly two exclusive
main-thread running-CPU categories. Its cycle pair includes running control
and pump work while naturally excluding sleep. One pair wraps each warm
call. A native clock microbenchmark will report per-read cost and resolution.
The verdict will use the ratio of warm CPU sums to cycle CPU sums over the
same cycles, with the clock-read allowance and positive whole-bundle CPU
delta charged against the warming numerator. Raw wall P50/P99 remain the
independent intrusion check. This is a running-CPU fraction, not busy-wall time.

Source proof: `data/E26/e26-final09-source.json`.
Raw whole-host rows must call outside processes `outside`. Raw files over
1 MB are losslessly compressed as `.json.gz`. Recorded hashes refer to the
decompressed bytes. Native traces stay under `.local/evidence/E26/`.

## Clock-fix overlap

E24's `horizon-13-lease-raw-clock-r1.patch` has SHA-256
`623d9b9614468b2ca180822f29ed772de68b66d4c306e7beb3be3ad84698a33e`.
It replaces the Darwin clock implementation used by local authority checks.
That is inside the diagnostic's lease bucket. It also adds clock telemetry
to authority status and renewal reporting. The latter includes off-thread
work. It does not change the selected FINAL-09 control or its recorded hash.
The profile cannot establish the cost of the replacement raw-clock reads,
resampling or histograms without a separate measurement on that stack.

## Retained setup failures

`e26-diagnostic-r1` stopped at formatting. R2 passed formatting and four timer
tests, then exposed two boolean-expression grouping errors in the diagnostic.
R3 passed those gates again and stopped at strict Clippy's condition-block
rule. These are diagnostic implementation failures, not engine results.

R4 found one remaining grouped boolean in the hub. R5 stopped before its
queued formatter ran, while a broader repair attempt was being discarded.
The failed repair source is archived privately. R6 reached the game crate's
strict check and found a redundant pair of parentheses and two block-valued
match inputs. It also saw the shared gate's script change during execution.
The reviewer confirmed the gate error's cause. The failed record is unchanged.
Patch 15 contains the narrow syntax repairs. Patch 16 adds reader-only inbox
and acknowledgement timers. Patch 17 excludes worker-completion waits in
`pump_chunks` and records the excluded pacing wait explicitly. The whole
unchanged upstream wait call is excluded, including its small queue handoff.
That makes the additions fraction conservative. Patch 18 separates worker
completion waiting from ordinary pacing sleep. R7 is queued on the current
gate, before its first Cargo command.

Each timer row belongs to the completed tick it names. Its pacing wait is
the wait immediately before that tick. The source order is raw tick, tick
recorder, pacing work and waits, then the next raw tick. Both sleep and
`Streaming::wait` therefore sit outside the raw E11 tick's wall interval.
The report will give their per-row P50/P99, maximum and nonzero count beside
busy time. They contribute zero time inside the raw-tick P99 clock, by source
placement. They can delay the following tick's start. No wait or limiter time
is subtracted from any raw tick. Worker demand may come from readers or
prewarming; the current counters do not attribute that cause, so waiting is
not part of the additions fraction.

## Planted-cost control

The separately named positive engine is
`.local/bin/horizon-e26/e26-planted-1`, SHA-256
`632a692a270fc7a92942a6d3917ea47ab4edef431213ab2d31517b33e39718cb`.
The negative diagnostic is `.local/bin/horizon-e26/e26-timers-1`, SHA-256
`216f1d247f41610fc83437f5fa8e71da7dd4ade54eca75014d3b2e19f955b111`.
Both passed the release gates and executable-snapshot checks.

| Positive control, 100 players | Result |
| --- | --- |
| Complete retained timer rows | 305, with one pulse each |
| Pulse P50 / P95 / P99 | 200.083 / 200.125 / 200.333 us |
| Required median range | 200 to 220 us |
| Dropped timer rows | 0 |
| Largest measured outside process | 0.08524 core |
| Server and bot limiter stops | 0 / 0 |
| Worker wait P50 / P99 | 0 / 0 ms |
| Pacing sleep P50 / P99 | 44.695 / 45.963 ms |

All 100 players remained online and every tile retained 64 pigs and ten
readers. The timer sum equals busy time. The artificial pulse appears in
its own category. Including it gives a 70.3041% instrumented additions
fraction. Removing only its measured cost from both numerator and denominator
gives a same-row counterfactual 69.1722%, a rise of 1.1320 percentage points.
That counterfactual is not a separate uninstrumented run. These large fractions
at modest load are calibration data, not the 650-player M4 result.

Verdict: the planted-cost control passes its declared tolerance. Raw data are
`data/E26/e26-planted-control-1.json.gz`, with decompressed SHA-256
`636c7619137ceddecdcc2bcd7d7dbd92e35f1f6d6624ab718d9d53f09c20597e`.
The recomputation is `data/E26/e26-planted-summary-1.json`.

`e26-ab-a1` stopped during bot joining, before warmup or measurement. The host
gate allowed an owned build while E26 held its own window. E26 interrupted
the fixture, retained its partial data and released the window. It supplies
no A/B observation. A shared process lock now serializes the whole E26 build
pipeline and every E26 cell, including their queue waits and cleanup. The
existing host gate still schedules E24 and the other builders.
The reviewer then changed the shared gate so every build waits for every
window, including its own, and every window drains every running build.
E26 uses neither an in-window build override nor its interrupted A1 numbers.

## Source and publication checks

The runtime-free timer module stays in Horizon's core crate. Its callers own
recording and select the one main thread to enable. No runtime or worker pool
was added. The crate-layout audit reported zero missing module docs and zero
unsafe blocks without a safety comment in this repository. The root crates
were not changed by E26, and no root Rust gate is claimed.

`data/E26/e26-preflight-audit-1.json` checks the diagnostic source, the frozen
internal dependency export and the original logical index. It also checks
1,106 outside-name entries and records decompressed-byte hashes for the first
16 evidence files. All those checks pass. The 16 retained PIDs from the
interrupted A1 fixture have exited.

## E26b continuation

E26b took over the owned `.local/src/horizon-tiles` checkout on 2026-11-09.
All 62 compiled-source files match `e26-lite-warm-source.json`, tree
`8c686f3e0b3619982e754d4dd1fe2a596c7c1777`. The current source is FINAL-09
plus patch 10 and the lite timers, with the archived internal dependency
overlay. The logical Git index is unchanged. Its raw bytes changed because
of index metadata, but `git ls-files --stage -z` is byte-identical to the
archived original index, SHA-256 `36481618f408f3473eb23516e9c03091391031de044aa44f9166ea48bba63cc0`.
No source reset or rebuild is needed for the first two comparisons.
The frozen engines match their build records: lite `92af479e`, lite plus
warm-tick `06b3b567`, untimed warm-tick `ea453786`.

Clock calibration attempt 1 failed while waiting for the window and supplies
no clock observation. Attempt 2 remains queued as PID 90363. It holds the
E26 operation lock, so the lite campaign queues behind it without a second
E26 window request. The fixed lite order remains A/B/B/A, B/A/A/B, using
fresh controls and the predeclared 650-player D9 fixture. Attempt 2's result
will supply the calibration instead of attempt 1. Every attempt is retained.

Next steps are the lite verdict, untimed patch-10 A/B qualification with a
separate lite warming comparison, then a final lease-r2 plus qualified
warm-tick freeze. The reviewer must reproduce each result before acceptance.

Patch-10's method is frozen in `data/E26/e26-patch10-plan-1.json` before any
of its cells. It has a fixed eight-cell untimed comparison against unchanged
FINAL-09, then an eight-cell lite comparison with timers in both arms.
The latter compares like-for-like warming CPU, not wall-clock warming from
the full timer bundle. Each timed arm is also checked against its untimed
arm for intrusion. The adapter only changes frozen arm identity, proof
format and whether to collect the two lite timers. The D9 kernel, selection,
CPU caps, routes and gameplay gates stay unchanged. There is no new capacity
claim from this 650-player diagnostic. A frontier claim requires its own
closed bracket and the final stack still needs the reviewer's recheck.

The continuation adapter and verdict arithmetic have eight load-free Python
checks, all passing, beside E26's six existing checks. They include the exact
5% boundary, every cycle and warm-call read, the positive main-CPU charge,
invalid calibration and intrusive timers. A first adapter test exposed an overly
broad AST condition match; it was fixed before any patch-10 cell ran.
The final-source preflight confirms horizon-13 r2 applies cleanly to the
private untimed patch-10 index. That patch imports `RenewalSchedule` and
`RenewalAction`, so its final build must use E24 r2's frozen internal crate
closure, not E26's earlier archived dependencies. All 35 current internal
files still match the r2 build record. Snapshot and verify that closure
before building; do not compile against a mutable shared dependency tree.

Calibration attempt 2 completed and passed both measured-phase noise checks.
Its 32 batches of 100,000 native thread-CPU reads have a maximum unsubtracted
batch mean of 170.16458 ns per read. The reported clock resolution is 42 ns;
the observed minimum positive step is 41 ns, with no zero steps in 100,000
resolution samples. The declared allowance is therefore 213 ns per read,
the ceiling of the maximum batch mean plus one reported quantum. The batch
includes loop and black-box work, and is an empirical conservative allowance,
not a global worst-case latency bound. The probe hash matches the lite
release record. Data: `data/E26/e26-clock-calibration-2.json`.
The fixed lite campaign has started its A1 attempt and retains every cell.

Before any patch-10 cell, its adapter was revised to verify the original
E21 proof at `data/E21/stack-proof.json`. The retained untimed bundle does
not contain `source-proof.json`; both lite bundles do. All three proof hashes
match their build records in the new load-free check. No frozen bundle was
edited. The plan records the revised adapter hash before any observation.

The lite campaign's A1 attempt completed and released its window. B1 is
next. The serial campaign invokes `scripts/e26-lite-cell` in the order from
`e26-lite-plan-1.json`, then `scripts/e26-lite-results` with calibration 2
and output `e26-lite-summary-1.json`. Do not launch another copy while its
controller is live. Each completed cell has its own raw record, decompressed
hash, executed adapter and kernel, and window evidence in `data/E26/`.

A1 and B1 both pass the retained-data validity checks and the unchanged D9
fixture gates. B1 has 1,208 complete lite cycles with valid partitions,
read counts and sequence coverage. B2 is next in the fixed order. No lite
verdict is drawn from this first pair, and the order remains unchanged.

The first lite block, A1/B1/B2/A2, completed without a skipped or replaced
row. The serial controller has moved to B3, followed by A3/A4/B4. The full
comparison and charged verdict remain pending until those four rows finish.

### Lite verdict

The fixed eight-cell lite campaign completed with four valid rows per arm,
unchanged harness hashes and the declared order. All four timed windows
have complete partitions and correct read counts. There are 4,828 retained
cycles. Their cycle CPU sum is 78.705107515 s and warming CPU sum is
4.165718357 s, giving a window ratio of sums of 5.2928183%.

The allowance charges all 210,482 reads at 213 ns each against the warming
numerator, a total of 0.044832666 s. Selected main CPU's B-minus-A mean is
negative, so its full positive-delta charge is zero. Corrected warming CPU
is 4.120885691 s, or 5.2358555% of the unchanged cycle CPU denominator.
This is strictly above 5%. The declared lite verdict is therefore failed
and now verified by the reviewer. It proves the warming-only lower bound under this
method, not the numerical sum of every addition or its cost on lease-r2.

| Required intrusion metric | A mean | B mean | B minus A | Maximum within-arm spread |
| --- | ---: | ---: | ---: | ---: |
| Selected main cores | 0.33740 | 0.32607 | -0.01133 | 0.01920 |
| Raw P50 ms | 16.201 | 15.972 | -0.228 | 1.318 |
| Raw P99 ms | 35.774 | 39.826 | 4.052 | 8.208 |

All three mean shifts are inside their observed within-arm spread. These
four-row ranges are descriptive noise, not statistical confidence bounds.

| Timed row | Tile | Window warm/cycle CPU | Per-cycle fraction P50 | Per-cycle fraction P99 |
| --- | --- | ---: | ---: | ---: |
| B1 | a | 5.36397% | 5.88317% | 9.00443% |
| B2 | a | 5.39299% | 5.89060% | 8.66402% |
| B3 | a | 5.30760% | 5.70011% | 8.33885% |
| B4 | d | 5.11065% | 5.45335% | 7.94239% |

The verdict number is the charged window ratio of sums, not an average of
these per-cycle fractions. The table gives the uncharged observations.
All four B rows pass the unchanged D9 fixture gates. A3 retains tile a's
51.574 ms raw P99 failure, while its selected tile d passes, and remains in the comparison;
there is no capacity acceptance or replacement observation here.
Data and recomputation are `data/E26/e26-lite-summary-1.json` and
`scripts/e26-lite-results`, using `e26-clock-calibration-2.json`.
The reviewer was sent the milestone directly. Patch-10's fixed untimed
campaign has now reached A1, followed by its separate two-arm lite campaign.

### Reviewer runtime reproduction

Run this from the repository root with a fresh name:

```sh
python3 -u -B scripts/e26-lite-cell --name repro-e26-lite-review-b1 --arm B --bins .local/bin/horizon-e26/e26-lite-r2
python3 -B scripts/e26b-repro-lite repro-e26-lite-review-b1 --out repro-e26-lite-review-b1-summary.json
```

For the second fresh runtime cell, replace both `b1` names with `b2`.
The queue owner is `E26`, selected by the adapter itself. Do not wrap the
command in a separate `quiet` request and do not use queue owner `reviewer`
for this adapter. It takes `.local/evidence/E26/operation.lock` before its
quiet request, so simultaneous reviewer and builder invocations serialize
without sharing a live E26 window. Each cell releases its own window after
cleanup. Both cells use the frozen lite bundle, 650 players, 30-second warmup,
60-second measurement and the original busiest-tile selection.

The single-row helper uses the same query-bracketed complete-cycle subset
as the builder's verdict. It reports warm/cycle CPU sums and per-cycle
P50/P99, charges every read at 213 ns, and also charges the full positive
single-B main-CPU difference from the four A rows' mean against warming.
It compares that B row's main CPU, raw P50 and raw P99 to the A-row mean;
each absolute difference must be no larger than the four A rows' range.
Those A-only spreads are 0.01919594465 core, 1.318 ms and 7.107749 ms.
This is deliberately a single-row check against retained controls, not a
new balanced eight-cell experiment. `corroborates_failure` requires all
three intrusion checks and a charged ratio strictly above 5%; otherwise
the row does not independently corroborate the failure. A3's separate
capacity-gate caveat remains in the output.

The reviewer independently recomputed all rows from the four retained B
records, rather than only the query-bracketed subset. The result is 5.2914%
before and 5.2344% after the 211,120-read charge, with every B record above
5%. Combined per-cycle P50 is 5.71% and P99 is 8.66%. The reviewer also
confirmed that the timer covers our `tile_warm_generation` routine.

The two fresh runtime rows are retained without replacement:

| Fresh reviewer row | Charged warm/cycle CPU | Intrusion against retained A spread | Failure corroboration |
| --- | ---: | --- | --- |
| `repro-e26-lite-review-b1` | 5.3591% | outside | no |
| `repro-e26-lite-review-b2` | 5.6875% | inside | yes |

B1's main CPU is 0.0204 core below the retained A mean, just beyond the
0.0192-core A spread. Its warming ratio is above 5%, but that row does not
independently corroborate the failure under the declared intrusion test.
B2 passes the intrusion test and corroborates the failure. The reviewer
recorded the M4 tick item as failed and verified in `progress.md`.
The summaries are `data/E26/repro-e26-lite-review-b1-summary.json` and
`data/E26/repro-e26-lite-review-b2-summary.json`. No capacity or lease-r2
cost claim follows from these unchanged FINAL-09 reproductions.

Final-stack preflight is retained in `data/E26/e26-final-preflight-1.json`.
FINAL-09 plus horizon-13 r2 and patch 10 applies cleanly in a private index,
as does the clean lite diagnostic on top. Patch 21 is exactly the five-file
net diff from FINAL-09 to the already proved lite tree. It contains no full
timer hooks and adds no new measurement behavior. This prepares the final
candidate's requested lite row without changing the owned checkout or
starting a build before patch-10 qualification. The final-stack lite row
will measure warming directly on the clock-fixed candidate; it cannot
certify the cost of every addition that the lite recorder does not cover.

The intended final canonical patch order was also reconstructed from the
pin in a fresh private index, including every recorded measurement-patch
option. It exactly reproduces both preflighted trees. The ordered proof is
`data/E26/e26-final-order-proof-1.json`; the checkout was not changed.

The renewal-r2 internal source closure is frozen at
`.local/evidence/E26/renewal-r2-internal`, with its 38-file hash manifest in
`data/E26/e26-renewal-r2-internal-source.json`. It includes the 35 source
files verified against E24 r2 and the three retained workspace files. No
shared source was edited. The final source and freeze helpers require an
explicit interpreted patch-10 qualification record before activation or
build. Untimed and lite releases have separate names, gated sources and
snapshot metadata. At this preflight point, ten continuation Python tests
and six original tests passed; final-source Rust gates had not yet run.

### Continuation handoff state

Both fixed patch-10 comparisons and both final bundle builds are complete.
The old serial controller and build controllers have exited. Do not restart
the qualification campaign. Actual frozen identities and the exact
reviewer-only command are in "Final bundle handoff" below.

The checkout is restored to the untimed final source, tree
`15e9cdaf58c09ea9ad514655fc745bff73d2350c`, with proof in
`data/E26/e26-final-stack-source-restored-r1-source.json`. All 62 selected
files and all three logical-index comparisons pass. Dependencies are the
38-file frozen renewal-r2 closure, not the mutable shared root tree.
No final-stack frontier, lite or soak cell was run by the builder.

Untimed patch-10 A1 is valid, passes the D9 fixture gates and contains no
lite or full-timer collection. The retained warm candidate and unchanged
control have identical 34-file internal dependency manifests, also equal
to E26's archived FINAL-09 closure. E21's build helper checks those hashes
before and after its gates. During the campaign, B1 waited behind E25's
30-minute candidate soak; no observation was selected or replaced.

The first untimed patch-10 candidate, B1, completed after the reviewer began
the fresh lite runtime reproduction. Its original E21 source-proof path
passed the adapter's checks. The remaining rows followed the unchanged
order; no qualification verdict was drawn from that first pair.

Before the first candidate observation, the qualification interpretation
was recorded in `data/E26/e26-patch10-qualification-rules-1.json`.
Warming CPU milliseconds per cycle is the direct optimisation target.
Its mean reduction must exceed the larger within-arm range, with both
timer-intrusion checks inside noise. Untimed CPU and each tile's raw P50/P99
must show no positive regression beyond their own larger within-arm range,
and all four untimed candidate rows must pass the original fixture gates.
Whole-CPU changes inside noise are neutral, never CPU wins. A qualified
warming reduction does not imply an aggregate CPU or capacity improvement.

After interpreting patch 10, record `e26-patch10-qualification-1.json` with
`qualified` and the summary file's SHA-256 in `summary_sha256`. Only a true
qualification enables the final source helpers. Planned fresh identities:

```sh
python3 -B scripts/e26b-final-source emit --name e26-final-warm-r1 --from-proof e26-lite-warm-source.json
# Apply the emitted source patch through apply_patch, then prove those bytes.
python3 -B scripts/e26b-final-source check --name e26-final-warm-r1 --from-proof e26-lite-warm-source.json
python3 -u -B scripts/e26b-final-build --name e26-final-warm-r1 --proof e26-final-warm-r1-source.json
```

The build helper owns one serial `host-window build E26` chain, release
profile only, incremental off. It runs formatting, 13 lease tests, 3 warm
tests, strict scoped Clippy and release linking, then freezes the E24 r2
support binaries with the new tile. Send its path and hashes to E25 and
the reviewer only when the build record says complete. Results remain
candidate evidence until the reviewer reproduces them.

The separate final lite bundle uses these commands:

```sh
python3 -B scripts/e26b-final-source emit --lite --name e26-final-warm-lite-r1 --from-proof e26-final-warm-r1-source.json
# Apply the emitted source patch through apply_patch, then prove those bytes.
python3 -B scripts/e26b-final-source check --lite --name e26-final-warm-lite-r1 --from-proof e26-final-warm-r1-source.json
python3 -u -B scripts/e26b-final-build --name e26-final-warm-lite-r1 --proof e26-final-warm-lite-r1-source.json
```

It adds the three lite tests and the native clock-probe gates. The reviewer,
not the builder, will run the final-stack frontier, lite and soak cells.
No builder final-stack load is authorised by the latest continuation.
The lite rows report warming only, not the cost of every addition.

### Patch-10 qualification result

Both fixed eight-row comparisons completed with valid observations and
unchanged within-comparison driver and harness identities. All 16 rows
retain 64 pigs and 65 readers per tile. All four untimed candidate B rows
pass the original D9 gates. No observation was skipped or replaced.

| Untimed metric | FINAL-09 A mean | Patch-10 B mean | B minus A | Maximum within-arm spread |
| --- | ---: | ---: | ---: | ---: |
| All-server cores | 3.31566 | 3.26187 | -0.05378 | 0.28734 |
| Mean tile cores | 0.56139 | 0.53298 | -0.02841 | 0.02525 |
| Selected process cores | 0.56728 | 0.53827 | -0.02901 | 0.03203 |
| Selected main cores | 0.32207 | 0.30955 | -0.01252 | 0.02395 |
| Selected raw P50 ms | 15.576 | 15.863 | 0.287 | 2.146 |
| Selected raw P99 ms | 35.305 | 34.316 | -0.989 | 12.326 |

Mean-tile CPU's reduction exceeds its declared spread. Aggregate-server,
selected-process and selected-main CPU changes remain inside noise and are
not claimed as wins. No CPU metric or any individual tile's raw P50/P99
has a positive regression beyond its own spread. Tile c's positive shifts
are retained: P50 1.07249 ms against a 2.25850 ms spread, and P99 4.34522 ms
against 14.17825 ms. These ranges are descriptive, not confidence bounds.

| Lite warming metric | FINAL-09 A | Patch-10 B |
| --- | ---: | ---: |
| Complete cycles | 4,827 | 4,829 |
| Warm CPU ms per cycle | 0.85665 | 0.05837 |
| Window warm/cycle ratio of sums | 5.28754% | 0.37550% |
| Combined per-cycle fraction P50 | 5.68430% | 0.36937% |
| Combined per-cycle fraction P99 | 8.50256% | 0.66350% |

The mean per-row warming reduction is 0.798285 ms per cycle, above the
larger within-arm range of 0.122132 ms. Both timed-versus-untimed intrusion
checks pass for main CPU and raw P50/P99. The timed A1 control retains a
separate raw-tail capacity-gate failure; it remains in the comparison.
The qualification requires the untimed candidate's gates, not a replacement
for that control failure.

The builder therefore qualifies patch 10 as a warming-cost optimisation
under the recorded rules, pending the reviewer's reproduction. The data do
not establish a new frontier or the total cost of every addition. A capacity
claim still requires two passes at the highest step and two failures one
step above. The final clock-fixed stack needs its separate recheck.
Recomputation is `data/E26/e26-patch10-summary-1.json`; the interpreted
decision, including every per-tile regression check, is
`data/E26/e26-patch10-qualification-1.json`.

The reviewer was sent this milestone. The next step is the preflighted
FINAL-09 plus horizon-13 r2 plus patch-10 tile candidate, followed by its
separate lite diagnostic. No final-stack build or runtime result is claimed
at this point.

The reviewer independently recomputed the raw lite records and confirmed
patch 10: A 5.2861% and 0.8564 ms per cycle, B 0.3752% and 0.0583 ms per
cycle. Every B row is between 0.36% and 0.39%. The reviewer authorised the
final candidate build with these complete-bundle contents:

- FINAL-09 plus horizon-13 renewal-r2 plus patch 10 tiles;
- bounded store `9636fa6d8089b071be241437e79eb2942a07dc756039b394e73cc8f705e26d0e`;
- rebuilt matched reactor-1 proxy `80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378`;
- unchanged launcher, sidecar, bots and e4 client from the r2 bundle.

Both complete bundles will retain FINAL.json, LEASES, canonical.json,
candidate.json and the same executable layout as `lease-fixed-r2`.
Canonical metadata will contain every bundled file hash except its own
self-referential hash. Runtime, frontier and soak validation remain marked
pending reviewer reproduction. The reviewer will run two 650-player
frontier cells, two lite cells and one 30-minute soak. The builder must not
run those final-stack cells.

The untimed final source is now activated and byte-proved in
`data/E26/e26-final-warm-r1-source.json`, tree
`15e9cdaf58c09ea9ad514655fc745bff73d2350c`. Its release gate chain is next.

The untimed serial chain completed all Rust gates: formatting, 13 lease
tests, 3 warm-tick tests, strict scoped Clippy and release linking. The
foreground tool transport timed out after 300 seconds despite a longer
requested timeout. The native chain continued and linked successfully,
but the controller then lost its reporting pipe before freezing. That
attempt remains failed in `e26-final-warm-r1-build.json` with `Broken pipe`;
the complete original gate log remains private under `.local/evidence/E26/`.

No Cargo command was repeated. After the native controller exited, the
builder checked every end marker, test count, profile and unchanged source
proof, then froze the retained output under a fresh identity:
`.local/bin/horizon-e26/e26-final-stack-r1/`. Its build record explicitly
marks the gates as reused from the reporting-failed attempt, binds the
original record and log hashes, and says there were no new Cargo commands.
The complete bundle's snapshot and every canonical file hash pass.
The separate lite build then completed its own full serial gates and
snapshot, with the controller's output retained in a private file. It did
not repeat the reporting failure. No final-stack cell was run by the builder.

### Final bundle handoff

Untimed runtime bundle: `.local/bin/horizon-e26/e26-final-stack-r1/`.
Its canonical SHA-256 is
`05af23a804b2f9928ed4e497a7212705360f279c3965f5c80caba48d6061e85a`.

Separate lite runtime bundle: `.local/bin/horizon-e26/e26-final-stack-lite-r1/`.
Its canonical SHA-256 is
`20ded3f006cdccdd6d7254e76d40bc0c21135b94b32d20206b61bebb551e6e7c`.

| Executable | SHA-256 |
| --- | --- |
| Untimed tile | `9c3d0d0cab89e3bf474e73042854a7af9b7ac18baa60ba3a4391dbe8fc8f2f99` |
| Lite tile | `63ce2adcee2555f61cd1a2c3edfc9e1743386c0db6ade1c0b09d1d92c246de68` |
| Matched reactor-1 proxy | `80f8953a201a51a0cae84341080af0bd70e08586ad608475b3092b2c30d3c378` |
| Bounded store | `9636fa6d8089b071be241437e79eb2942a07dc756039b394e73cc8f705e26d0e` |
| Sidecar | `c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b` |
| Bots | `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d` |
| e4 client | `e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025` |

Each bundle has the same complete leased runtime layout as `lease-fixed-r2`.
Canonical `bundle_files` records every included file hash except the
canonical file's self-hash, 15 files for untimed and 16 for lite. It includes
the launcher, LEASES, FINAL.json, candidate.json, manifest, source proof,
proxy build/lock evidence and build record. Both snapshot contracts and
every recorded file hash pass. No harness change is needed for
`scripts/e20-frontier` or E24/E25's soak harness.

Untimed gates passed formatting, 13 lease tests, 3 warm tests, strict scoped
Clippy and release linking. Lite gates also passed 3 lite tests and native
clock-probe Clippy/linking. Eleven continuation Python tests and six
original Python tests pass. No root Rust crate was edited or root Rust gate
claimed by E26b.

Exact fresh reviewer lite commands, from the repository root:

```sh
python3 -u -B scripts/e26b-final-lite-cell --name repro-e26-final-lite-b1 --arm B --bins .local/bin/horizon-e26/e26-final-stack-lite-r1
python3 -u -B scripts/e26b-final-lite-cell --name repro-e26-final-lite-b2 --arm B --bins .local/bin/horizon-e26/e26-final-stack-lite-r1
```

These commands are documented, not executed by the builder. The wrapper
uses the original lite measurement adapter unchanged except for an honest
final-stack B-only control label. It guards the final source and selected
proxy/store identities. Its contract test proves the generated driver
differs only in that label. It retains 650 players, 30 seconds warmup,
60 measured seconds, D9, both CPU caps and the original selection rule.
Queue owner `E26` and the shared operation lock are handled internally;
do not wrap another quiet owner around it.

The final native clock probe is
`939d08c0ba7bf9391e190a37c8a075f205544da316ab281c861187f0bb640b28`.
It differs from the earlier calibrated probe image. No final calibration
was run, and the old 213 ns allowance is not claimed as verified for this
new image. Do not treat these final B-only rows as a matched comparison to
the earlier FINAL-09 A rows. Final frontier, lite and soak numbers belong
to the reviewer's fresh runs. Runtime validation remains pending.

Both paths and hashes were sent directly to E25 and the reviewer. E25's
handoff was data-only, not a request to start another load. The builder
restored the untimed source after freezing lite and ran the required
non-`--all` cleanup through the host scheduler. Cleanup completed with zero
MB removed; target directories were under cap and the project measured
22,671 MB. No source or frozen binary was deleted.

The final static audit confirms all 24 owned load records agree with their
retained decompressed-byte hashes. Both complete bundle hash maps and the
restored source/index checks pass. Final machine-readable handoff is
`data/E26/e26-final-handoff-1.json`. No commit, push or upstream action was
taken. No final-stack runtime result is claimed by the builder.
