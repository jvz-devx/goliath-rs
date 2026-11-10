# E29 measurements. The soak on the store that answers renewals early

Status: the memory line is open. The builder ran two soaks and the reviewer
reproduced the long one. All three kept every lease. `e29-soak-r1`,
30 minutes, fails the memory line. `e29-soak-r2`, 48 minutes with the memory
ruling applied as written, passes it by a margin that does not hold up: tile
c was 18.5% up two minutes before the end. The reviewer's
`repro-e29-soak-r2` fails it on tile d, +18.6%. **Verdict: not reproduced on
memory.** Every other line of the pass line holds in all three runs. No run
contained a barrier over 400 ms, so none tested the change against a stall
as long as the one that ended `e28-soak-r2`. No barrier-mode control ran. No
milestone claim is made.

The store change itself, its safety argument and its model are in
`E29-lease-renewal.md` on the `e29-lease-renewal` branch. This file holds only
what was built and measured from it.

## What was built

Bundle `final-r2-e29` is `final-r2` with `goliath-store` replaced and nothing
else. The guard accepts it only when it differs from `final-r2` in that one
executable.

| Item | Value |
| --- | --- |
| Store commit | `76fbd58ad549077827925a2adb4424b1797159fe`, branch `e29-lease-renewal` |
| Store SHA-256 | `6879c72fbb8806a343f621e9c44968c08082da4a098142b4983d7b80b7b7dd00` |
| Build | `cargo build --locked --release -p goliath-store --bin goliath-store`, final's store recipe: no profile overrides, two jobs, `CARGO_INCREMENTAL=0`, empty `RUSTC_WRAPPER` |
| Gates on that commit | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: all exit 0, 92 tests |
| Record | [build-final-r2-e29.json](data/E29/build-final-r2-e29.json) |

Two things to know about the build:

- The worktree was clean before the release build. Afterwards it held one
  untracked script outside the build inputs. Ten minutes later another builder
  edited three store files there without committing. Every source file that
  was built equals the commit's blob, and the freeze checks that. The measured
  store is the committed one, without those edits.
- My first gate run landed on the edited files, so it doesn't count. The gates
  in the table ran on a `git archive` export of the commit.

## The first run

`e29-soak-r1`: 425 players, `final-r2-e29`, pinned, direct bots, the standard
fixture with bulk logs on the store's volume. The store ran in its default
mode, so granted renewals were answered before the barrier.

| Line | Result |
| --- | --- |
| Loaded time | 1,800 s, complete |
| Authority stops, clock anomalies | none on any tile |
| Players online | 425 in all 1,009 samples |
| Owner audits | 29, zero violations, zero generation regressions |
| Cargo after checkpoint and recovery | 42 of 42, no duplicates, none missing |
| Crossings | in every minute, 376 to 382 a minute |
| TPS | 19.972 to 20.000 |
| Long-window raw tick P99, tiles a to d | 42.6, 43.3, 44.2, 40.9 ms |
| Whole window | 2,092 s, D12 valid, CPU 5 73.6% busy, largest outside process 0.143 core, restored |
| Memory, 10% line | **fails**: store +288%, tile b +18.4% |
| Verdict | failed, on memory only |

### Did the run test the change

Not against a long stall, and I want that read before anything else here.

| Store barriers in the loaded interval | `e28-soak-r2` (failed) | `e28-soak-r2-diag-logs` | `e29-soak-r1` |
| --- | ---: | ---: | ---: |
| Count | 24,462 | 38,886 | 11,021 |
| Over 50 ms | 124 | 31 | 78 |
| Over 100 ms | 32 | 0 | 23 |
| Over 200 ms | 3 | 0 | 5 |
| Over 400 ms | 2 | 0 | 0 |
| Largest | 462.9 ms | 88.8 ms | 337.4 ms |

The r2 figures run only to its stop at 1,216 s. This run's largest barrier
was 337 ms. The stall that ended r2 was a 463 ms barrier followed by a
181 ms one. So this run shows that renewals no longer wait behind barriers
of up to 337 ms. It does not show a tile surviving what killed r2.

The count fell because renewals no longer make batches. That is the change
working as described, and it is why the count can't be compared across the
columns.

### Renewals

| | `e28-soak-r2` | `e28-soak-r2-diag-logs` | `e29-soak-r1` |
| --- | ---: | ---: | ---: |
| Renewals sent by the four tiles | not recounted | 28,779 | 28,778 |
| Replies over 200 ms | 18 | 0 | 0 |
| Slowest reply, measured at the tile | 475 ms | 117.3 ms | 15.5 ms |
| Answered early by the store | 0 | 0 | 28,778 |
| Journaled | all | 28,779 | 0 |
| In neither store trace | | 0 | 0 |

Every renewal a tile sent in the loaded interval is in the store's
`renewals.csv`, matched by request id. None went through the journal. Inside
the store the slowest early answer took 8.6 ms from receipt, and the slowest
wait from connection accept to receipt was 7.8 ms. Two answers took over 1 ms.

Five barriers lasted over 200 ms. Under the old path the renewals queued
behind them would have been slow. Here the slowest of 28,778 took 15.5 ms.

### Memory

Judged under the two rulings: resident plus swapped memory, from a baseline
after the store's receipt window fills. No measured process had any swapped
memory in this run, so the two measures agree. The host swapped out about
25,000 pages of other processes.

| Process | Minute 5, MiB | Minute 30, MiB | Growth | Under 10% |
| --- | ---: | ---: | ---: | --- |
| Store | 27.3 | 105.9 | +288.0% | no |
| Tile a | 355.6 | 356.2 | +0.2% | yes |
| Tile b | 294.5 | 348.6 | +18.4% | no |
| Tile c | 306.7 | 308.8 | +0.7% | yes |
| Tile d | 262.9 | 272.2 | +3.5% | yes |
| Proxy | 313.5 | 311.2 | -0.8% | yes |
| Sidecars | 3.3 | 2.8 to 3.0 | -10% to -15% | yes |

**The store.** This is a side effect of the change, and it breaks the
baseline rule as written.

- The receipt window holds 9,216 entries. With renewals journaled it filled
  335 s into the load, mostly with renewal receipts that carry no blob.
- With renewals out of the journal, only hand-over commits take a receipt,
  at 6.3 a second. The window filled at 1,343 s, minute 22.4.
- Each of those receipts carries a player blob of about 5.3 kB. A full
  window now holds 49.1 MB of blobs against 13.7 MB before, and the snapshot
  buffer is 51.1 MB against 15.7 MB. The store's plateau is about 106 MiB
  where it was 38 MiB.
- After the fill it rose from 101.6 MiB at minute 23 to 105.9 at minute 30,
  4.2% in seven minutes. That is too short to call flat.

The rule puts the baseline at the first per-minute sample after the fill and
judges 25 minutes from there. Here that is minute 23, which needs a 48-minute
run. The runner allows 45, so I capped the baseline at minute 15 before the
run. When the window was not full by then, the rule fell back to minute 5 and
the row says so. Receipts at each minute from 5 to 15 are in the row: 2,648
rising by about 375 a minute.

So the store's memory line is not judged fairly by this run, and it can't be
by any 30-minute run on this store at this player count.

**Tile b.** It stepped from 297 MiB at minute 18 to 351 at minute 23 and
stayed between 348 and 354 afterwards. Tile a made the same climb, from 283
to 356 MiB, in the first five minutes, before the baseline. Tiles c and d
did not. It reads as a one-time allocation that two tiles made at different
times, and the cause is unproven. Earlier Linux soaks could not have shown
it, because the tiles' resident size was falling as the host swapped them.
Under the rule it is a failure.

## The long run

`e29-soak-r2` is the same soak with the load extended so the ruling applies
as written. Its [registration](data/E29/e29-soak-r2-registration.json) was
written before the run. The store's window filled at 1,342.8 s, the baseline
is the minute-23 sample and the load ran 2,880 s.

| Line | Result |
| --- | --- |
| Loaded time | 2,880 s, complete |
| Authority stops, clock anomalies | none on any tile |
| Players online | 425 in all 1,617 samples |
| Owner audits | 47, zero violations, zero generation regressions |
| Cargo after checkpoint and recovery | 42 of 42 |
| Crossings | in every one of 48 minutes, 375 to 383 a minute |
| TPS | 19.954 to 19.971 |
| Long-window raw tick P99, tiles a to d | 43.7, 43.7, 42.9, 43.8 ms |
| Whole window | 3,289 s, D12 valid, CPU 5 73.0% busy, largest outside process 0.140 core, restored |
| Store barriers over 50, 100, 200, 400 ms | 126, 34, 5, 0 of 18,037; largest 328.0 ms |
| Renewals | 46,044 sent, all answered early, none journaled, none missing; slowest reply 19.8 ms at the tile |
| Memory, ruled window, minutes 23 to 48 | passes: every judged process under 10% |
| Memory, minute 5 to 30 | fails as in the first run: store +287%, tile b +17.0% |
| Bot check of the underlying cell | **flagged**: one late round in cohort b and one in cohort a |
| Verdict | passed on the soak lines, with the caveats below |

No barrier passed 600 ms, so no control ran.

| Process | Minute 23, MiB | Minute 48, MiB | Ruled growth | Minute 5 to 30, resident |
| --- | ---: | ---: | ---: | ---: |
| Store | 101.5 | 108.1 | +6.5% | +286.8% |
| Tile a | 354.4 | 338.4 | -4.5% | -0.4% |
| Tile b | 352.5 | 326.4 | -7.4% | +17.0% |
| Tile c | 295.5 | 321.9 | +8.9% | +6.1% |
| Tile d | 292.9 | 288.5 | -1.5% | +3.5% |
| Proxy | 312.9 | 314.1 | +0.4% | +0.1% |
| Sidecars | 3.3 | 3.3 to 3.5 | +0.7% to +6.1% | +0.4% to +2.4% |

The ruled figures are resident plus swapped. By minute 48 the host had
swapped out 33 to 108 MiB of each tile and of the proxy, so resident size
alone would have read far lower. The per-minute series for the store and all
four tiles is in the
[memory series](data/E29/e29-soak-r2-memory-series.json.gz) and the
[per-minute rows](data/E29/e29-soak-r2-every-process-minute-rss.json).

### The store is flat once its window is full

It rose from 101.5 MiB at minute 23 to 108.1 at minute 48. Receipt blobs
held 49.09 MB at minute 25 and 49.20 MB at minute 45. The snapshot buffer
went from 49.6 to 51.2 MB. That is a slope of about 0.26 MiB a minute, and
it was still there at the end, so "flat" is fair for 25 minutes and unproven
beyond.

### What the tile steps are

They are transparent huge pages, and the workload has no part in them. The
run sampled `/proc/<pid>/smaps_rollup` of every tile once a minute:

| Tile | Step | Anonymous memory | Of which huge pages | Chunks, readers, threads |
| --- | --- | ---: | ---: | --- |
| a | minutes 0 to 3 | 247.1 to 308.3 MiB | 90 to 248 MiB | unchanged |
| b | minutes 10 to 14 | 248.0 to 298.0 MiB | 76 to 264 MiB | unchanged |
| c | minutes 31 to 33 | 262.7 to 303.5 MiB | 124 to 248 MiB | 373 chunks, 42 readers, 26 threads throughout |
| d | none by minute 48 | | 42 MiB at minute 33 | |

At each step the huge-page share more than doubles while file-backed and
shared memory stay put, and placed players move only with the crossing
cohort (96 to 117 a tile). The host runs huge pages in `madvise` mode with
`max_ptes_none` at 511, so when the kernel collapses a range it fills every
untouched page in it. That raises resident size by 40 to 60 MiB a tile, once,
whenever the kernel gets to that tile. Tile d has not had its turn. I did not
trace which allocator call asks for huge pages.

So a step can land anywhere in a run. Tile c's landed inside the judged
window.

### Two caveats on the pass

**Tile c passes on its last two samples.** It stood at 350.3 MiB at minute
46, 18.5% over its baseline. Between minutes 46 and 47 the tiles lost 13 to
27 MiB with no matching rise in swap, and tile c ended at 321.9 MiB, +8.9%.
A run that ended two minutes earlier would have failed on tile c. The
reviewer's run did fail, on tile d, and the next subsection has the likely
cause of the release.

**The bot check flags the row.** Cohort b logged one late round 2,001 s into
the load and cohort a one at 2,806 s. All 425 bots stayed online and the bot
core was 39% busy. Both late rounds coincide with a burst of swapping on the
host: 156,000 pages went out between minutes 33 and 34, including 13 MiB of
the bots themselves.

The swapping is the harness's own doing. The cell driver's resident size
grows by about 100 MiB a minute: 138 MiB at the start of the load, 3.2 GiB
at minute 30 and 5.0 GiB at minute 48, in this run and the first alike.
Available memory fell from 10.6 to 5.7 GiB. That explains the swapping the
earlier Linux soaks reported. I have not found which sampler holds the
memory, and I did not change it, since every row tonight should come from
one harness. It should be fixed before the next soak.

### Reviewer reproduction: not reproduced on memory

This subsection is the reviewer's result, read from the reviewer's own rows
with the same readback scripts. `repro-e29-soak-r2` ran the registered long
soak on the same bundle.

| Line | `repro-e29-soak-r2` (reviewer) | `e29-soak-r2` (builder) |
| --- | --- | --- |
| Loaded time, baseline | 2,880 s, minute 23 (fill at 1,342.3 s) | 2,880 s, minute 23 (fill at 1,342.8 s) |
| Authority stops, clock anomalies | none | none |
| Players online | 425 in all 1,615 samples | 425 in all 1,617 samples |
| Owner audits | 47, zero violations | 47, zero violations |
| Cargo | 42 of 42 | 42 of 42 |
| Crossings a minute | 375 to 384, every minute | 375 to 383, every minute |
| TPS | 19.986 to 20.000 | 19.954 to 19.971 |
| Long-window raw tick P99 | 41.8, 42.1, 40.1, 39.4 ms | 43.7, 43.7, 42.9, 43.8 ms |
| Barriers over 50, 100, 200, 400 ms; largest | 112, 24, 5, 0 of 18,089; 358.3 ms | 126, 34, 5, 0 of 18,037; 328.0 ms |
| Renewals | 46,029, all early, slowest 20.1 ms | 46,044, all early, slowest 19.8 ms |
| Bot check | clean | one late round in two cohorts |
| Whole window | D12 valid, CPU 5 72.8% | D12 valid, CPU 5 73.0% |
| Store, ruled window | +5.3% | +6.5% |
| Tiles a, b, c, d, ruled window | +4.5%, -2.4%, -2.4%, **+18.6%** | -4.5%, -7.4%, +8.9%, -1.5% |
| Memory line | **fails** on tile d | passes |

So everything reproduces except the memory line, and that one differs for a
reason the two runs make plain. Each tile makes one climb of 28 to 73 MiB,
and when it comes is not fixed:

| Tile | Reviewer's run: climb, anonymous memory, huge pages | Builder's run: climb, anonymous memory, huge pages |
| --- | --- | --- |
| a | minutes 0 to 2, 257 to 285 MiB, 94 to 202 MiB | minutes 0 to 3, 247 to 308 MiB, 90 to 248 MiB |
| b | minutes 0 to 7, 249 to 322 MiB, 48 to 274 MiB | minutes 9 to 14, 247 to 298 MiB, 64 to 264 MiB |
| c | minutes 10 to 17, 239 to 303 MiB, 38 to 258 MiB | minutes 26 to 33, 261 to 304 MiB, 98 to 248 MiB |
| d | minutes 31 to 37, 256 to 303 MiB, 132 to 224 MiB | none by minute 48, huge pages 24 to 40 MiB |

Anonymous memory here includes what was swapped. In the reviewer's run tile
d's climb fell inside the judged window and stayed: 292 MiB at minute 23,
350 at minute 38, 346 at minute 48. In the builder's run tile c's climb fell
inside it too, and the pass came only from the late release described
above.

That release now has a likely cause. In the builder's run the three tiles
that had climbed lost 18 to 29 MiB between minutes 46 and 48 while their
huge pages fell from 232 to 248 MiB at minute 23 to 174 to 186 MiB at the
end. Tile d, which had not climbed, lost nothing. That is what the kernel
does when it splits huge pages under memory pressure and drops the parts
nobody wrote to. The reviewer's run shows no such drop.

So the 10% line, read between two samples, is decided by whether a
huge-page climb or its undoing lands in the window. It says nothing about a
leak either way. Over the reviewer's run, once a tile has climbed it stays
within a few MiB: tile b 320 MiB at minute 23 and 311 at minute 48, tile c
308 and 300.

### A smaller receipt cap

A proposal only. Nothing was changed.

The cap exists so a retried command still finds its receipt. The store
diagnostics size that need as the larger of 27 times the busiest second and
the busiest 27 seconds, and ask for a cap of at least four times that plus
one.

| | First run | Long run |
| --- | ---: | ---: |
| Busiest second, mutations | 19 | 16 |
| Retry budget | 513 | 432 |
| Cap the four-times rule needs | 2,053 | 1,729 |
| Margin of today's 9,216 | 18.0 times | 21.3 times |

With renewals in the journal the same fixture measured 32 mutations in its
busiest second, a budget of 864 and a need of 3,457. Four tiles renewing
every 250 ms made 16 of those mutations a second, so 9,216 was sized for
traffic that no longer reaches the window.

A cap of 2,304, a quarter of today's, keeps a margin of 4.5 times in the
first run and 5.3 in the long one. At 6.3 commits a second it fills about
four minutes into the load, before the minute-5 baseline, so a standard
30-minute soak would judge the store fairly again. It would hold about
12.3 MB of receipt blobs instead of 49.1 MB, and the snapshot buffer should
shrink in step, which puts the store near 30 MiB instead of 106.

What that leaves open: the busiest second came from two runs at 425 players.
A higher player count or a burst of hand-overs raises it, and the cap should
be set from the frontier's worst second with the four-times rule, not from
these two numbers.

## Harness changes this run needed

- The soak control, cell and readback take `e29-` names and an E29 data
  folder. The cell derives the allowed executables from the bundle's
  derivation instead of naming one.
- `scripts/e29_renewals.py` and `scripts/e29-soak-report` read `renewals.csv`,
  look up every tile renewal in it and in the request trace, and fail when a
  default-mode run shows no early renewals or a granted renewal is in neither
  trace. Run on the previous soak's evidence it reproduces that soak's
  published barrier and renewal figures.
- The memory rule is `scripts/e28_soak_memory.py`. It reads the store's own
  memory trace for the fill.
- The launcher takes opt-in store arguments, empty by default. The soak
  kernel sets `--renewals barrier` only for a run named as a barrier control
  on this bundle, and records the store's arguments in the row.

Two readers broke on the new trace shape, after the run had finished:

- The availability reader lists every gap of over 200 ms between batch
  starts. With renewals gone from the batch trace the store is idle between
  hand-overs, and 3,175 gaps qualified. That pushed the native summary past
  1 MB, so it was stored compressed.
- The soak cell read the summary only under its uncompressed name, found
  nothing and recorded the run as failed.

The run's own record has no error. I fixed both readers, with tests, and
judged the run again from its retained evidence with
`scripts/e25-soak-recollect`, which runs nothing. The first collection
record is kept as
[e29-soak-r1-collection-first.json](data/E29/e29-soak-r1-collection-first.json).
[e29-soak-r1-run.json](data/E29/e29-soak-r1-run.json) still says `failed`,
because that is what the control command returned.

## Files

- [plan](data/E29/e29-soak-r1-plan.json),
  [result](data/E29/e29-soak-r1-e25-result.json),
  [store and renewals readback](data/E29/e29-soak-r1-store-renewals.json)
- The long run: [registration](data/E29/e29-soak-r2-registration.json),
  [plan](data/E29/e29-soak-r2-plan.json),
  [result](data/E29/e29-soak-r2-e25-result.json),
  [store and renewals readback](data/E29/e29-soak-r2-store-renewals.json),
  [raw row](data/E29/e29-soak-r2-raw.json.gz),
  [audits, with the per-minute memory by kind](data/E29/e29-soak-r2-audits.json.gz)
- The reviewer's long run: [result](data/E29/repro-e29-soak-r2-e25-result.json),
  [store and renewals readback](data/E29/repro-e29-soak-r2-store-renewals.json),
  [audits, with the per-minute memory by kind](data/E29/repro-e29-soak-r2-audits.json.gz)
- First run: [raw row](data/E29/e29-soak-r1-raw.json.gz),
  [audits](data/E29/e29-soak-r1-audits.json.gz),
  [memory series](data/E29/e29-soak-r1-memory-series.json.gz),
  [per-minute memory of every process](data/E29/e29-soak-r1-every-process-minute-rss.json)
- [smoke row](data/E29/e29-smoke-100-r0.json.gz): 100 players, a check of the
  harness and the new store before the soak, not a measurement.

Native traces stay under `.local/evidence/E29/soak/`.

## Unproven

- That a tile keeps its lease through a barrier of 460 ms or more. No such
  barrier occurred.
- That the change is what kept the leases on this machine. No control ran.
  With largest barriers of 337 and 328 ms a barrier-mode run might well have
  survived too: `e28-soak-r2` survived a 407 ms barrier before the one that
  ended it.
- That the store stays flat past 25 minutes after its window fills.
- That the release at minute 46 of the builder's long run was the kernel
  splitting huge pages. The huge-page figures fit; no kernel counter was
  recorded.
- Which allocation asks for huge pages in a tile. The step itself is
  measured; its trigger is not.
- That the two late bot rounds come from the host's swapping. They coincide
  with it, and nothing else in the row stands out.
- Which part of the harness grows by 100 MiB a minute.
- Everything in the branch's own list: store restart under load, the M3
  campaign and the E15 and E16 latency rows on this store.

## What I'd ask the reviewer to decide

1. The long run is not a pass: the reviewer's reproduction fails on memory.
   What is left to decide is the rule.
2. How tiles should be judged. A huge-page step of 40 to 60 MiB is not a
   leak, and it can land in any window. Options: judge tiles on a slope over
   the window instead of two endpoints, start the tiles with huge pages off,
   or compare anonymous memory less huge-page fill.
3. Whether to lower the receipt cap as proposed above.
4. Whether the harness's memory growth must be fixed before any further soak
   row counts.

## Reproduction

These create only reviewer-owned `repro-*` results. The builder has not run
them.

```sh
export GOLIATH_WINDOW_OWNER=E28R
.local/bin/host-window build E28R -- python3 -B scripts/e28-soak-tests
python3 -u -B scripts/e25-soak-control --name repro-e29-soak-r1 \
  --bins .local/bin/goliath-e28/final-r2-e29 --data-dir docs/experiments/data/E29 \
  --cpu-mode pin --direct-bots --players 425 \
  --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
# After a full collection and release:
python3 -B scripts/e25-soak-readback --name repro-e29-soak-r1 --data-dir docs/experiments/data/E29 --owner E28R
python3 -B scripts/e29-soak-report --name repro-e29-soak-r1 --data-dir docs/experiments/data/E29
```

The long run needs its registration record first, under the new name, and
one more flag:

```sh
cp docs/experiments/data/E29/e29-soak-r2-registration.json docs/experiments/data/E29/repro-e29-soak-r2-registration.json
python3 -u -B scripts/e25-soak-control --name repro-e29-soak-r2 \
  --bins .local/bin/goliath-e28/final-r2-e29 --data-dir docs/experiments/data/E29 \
  --cpu-mode pin --direct-bots --players 425 --extended-memory-window \
  --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
python3 -B scripts/e25-soak-readback --name repro-e29-soak-r2 --data-dir docs/experiments/data/E29 --owner E28R
python3 -B scripts/e29-soak-report --name repro-e29-soak-r2 --data-dir docs/experiments/data/E29
```

The control, on the same bundle. The name must contain `-barrier`:

```sh
python3 -u -B scripts/e25-soak-control --name repro-e29-soak-r1-barrier \
  --bins .local/bin/goliath-e28/final-r2-e29 --data-dir docs/experiments/data/E29 \
  --cpu-mode pin --direct-bots --players 425 --store-renewals barrier \
  --campaign-plan docs/experiments/data/E28/soak-m3-protocol.json --owner E28R --collect-only
```

To rebuild and check the bundle from the branch's worktree at the commit
above:

```sh
cd <worktree> && CARGO_INCREMENTAL=0 RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 \
  $REPO/.local/bin/host-window build E28R -- \
  cargo build --locked --release -p goliath-store --bin goliath-store
sha256sum target/release/goliath-store
python3 -B -c "import sys; sys.path.insert(0, 'scripts'); import goliath_bundle as b; print(b.validate_server_stack(b.BUNDLES / 'final-r2-e29'))"
```

Keep failed and invalid rows. If a run is killed, run
`python3 -B scripts/goliath-cpu restore` before removing stale locks.
