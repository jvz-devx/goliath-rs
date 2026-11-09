# E29. Lease renewal without the durable barrier

Status: designed, modelled and implemented by the builder. Nothing here is
measured on real tiles and nothing is reproduced by the reviewer yet. No Folia,
Paper, GPL source or excluded research was read. Horizon and Meridian are
unchanged.

## Question

A tile keeps authority through a 1,000 ms lease with a 100 ms margin, renewed
every 250 ms (E16, cadence r2 in E24). Each renewal is a journaled mutation, so
its reply waits for the store's group-commit barrier. Can renewals stop
depending on the journal's slowest fsync without weakening the E16 safety
argument?

## Why it matters

The renewal budget follows from the schedule in `goliath-model`. A renewal goes
out 250 ms after the last granted send. The keeper's deadline is that send plus
900 ms, and the call must finish 50 ms earlier. That leaves 600 ms per call.

The store has one worker. A renewal that arrives while a batch is inside its
barrier waits for the rest of that barrier, then for its own batch's barrier. In
run `e28-soak-r2` on the second test machine the reviewer reports a 460 ms
barrier followed by a 180 ms one, so a renewal waited 640 ms against the 600 ms
budget and two tiles stopped. That run is not yet written up in E28 on this
branch, so the figures are quoted as reported. E28's diagnosis of the earlier
`e28-soak-r1-a1` loss shows the same shape with 409.5 ms and 167.5 ms barriers.
The Mac's retained soak maxima were 41.2 ms and 19.8 ms, so the coupling
stayed hidden there, apart from one 435 ms barrier during a limiter STOP in
E24 that cost a tile the same way.

The barrier tail is not bounded by anything the cluster controls. It comes from
the file system and the device under the journal. A tile's availability
therefore depends on the worst fsync of a shared volume.

## What a durable renewal buys today

Less than it costs. Since E16 a restarted store gives every recovered lease a
new deadline of a full lease from the end of recovery
(`State::restart_leases`). No deadline is ever written to disk. A renewal's
journal record therefore adds nothing to mutual exclusion after a crash. It
keeps two things across a restart: the renewal counter, and the retry receipt.

Renewals are also most of the journal's traffic in a quiet cluster. Four tiles
at four renewals a second are 16 journaled mutations and up to 16 barriers a
second.

## Options

### (a) Answer a granted renewal before the barrier and keep it out of the journal

The store extends the lease in memory and replies at once. A crash loses the
extension. The restarted store waits a full lease before it grants that tile
to anyone, and the lost grant ended before that.

Assumptions:

- Clocks. The same as E16: the tile's clock and the store's clock each advance
  at a rate within a known bound, before and after a restart. No clock is
  compared with another and no wall time is used.
- Crashes. A kill or a power cut can lose any unsynced suffix of the journal
  and all memory. The claim that created the lease must be on stable storage
  before any renewal of it is answered early.
- Restart. Every restart waits a full lease from the end of recovery, for every
  recovered lease, and recovery reads the directory the crashed store wrote.
  Starting from an older copy of that directory is outside the argument, as it
  already is for claims.

The cost is that the renewal counter and the renewal's retry receipt no longer
survive a restart. Neither is needed for exclusion. The details are under
"What changes for callers".

### (b) A longer lease

With a 2,000 ms lease, a 200 ms margin and a 500 ms cadence, the per-call
budget is 1,800 - 50 - 500 = 1,250 ms. The 640 ms wait would have survived.

It costs failover time. A crashed tile leaves between three quarters of a lease
and a whole one unexpired. E16 measured the store-side wait at 742 to 1,003 ms
for a 1,000 ms lease and 1,496 to 2,008 ms for 2,000 ms. E13 measured tile
replacement at 0.57 to 0.59 s, so the lease is already the longer of the two
and doubling it adds about 0.75 to 1.0 s to every tile failure. The wait after
a store restart doubles to 2 s as well.

It also only moves the line. Any barrier pair longer than 1,250 ms still ends a
tile. The model below shows both effects. With the measured tail profile a
2,000 ms lease cuts stalled-barrier stops from 48,765 to 82 per 100,000 runs.
With a worse disk it has 126,658.

### (c) A separate, cheaper durability path for lease state

A small second log, synced on its own, still waits for a device. On the second
test machine both volumes share one physical disk, and E28's probe saw a 4 KiB
append and fsync take up to 42 ms on an idle host from writeback alone. Putting
the lease log on memory-backed storage removes the wait and the durability
together, which is option (a) with more code. A second log would also need the
same change to the store's threading as (a), because a renewal still queues
behind the main journal's barrier in a single worker.

### (d) Other ideas considered

- Run the barrier off the worker thread, so a renewal waits only for its own
  barrier. The 640 ms case becomes about 460 ms. The coupling stays, and the
  journal file would be shared between threads.
- Renew every 125 ms. The budget grows from 600 to 725 ms. The coupling stays.
- Take the bulk logs off the store's volume (E28's fix 4). This shortens the
  tail and is worth doing anyway. It bounds nothing.

None of these removes the dependency. The builder found nothing better than
(a).

### Choice

Option (a), with the lease, margin and cadence unchanged. It removes the
renewal's dependency on the barrier, costs no failover time, and leans on a
restart rule the store has had since E16.

## Safety argument

Let s be the real time a tile sends a renewal and g, with g >= s, the instant
the store extends the lease. As in E16 the tile stops acting by
s + (L - margin) / (1 - rc). The reply promises that the store grants no other
fence for the tile before g + L / (1 + rs). With margin >= L (rc + rs) / (1 + rs)
the tile stops first. A reply that waits for no barrier changes neither time.
What has to hold is the promise, in two cases.

**The store keeps running.** The extension is written to the one lease value
that every claim reads, under the lock that also orders claims, takeovers and
releases. `ClaimLease` is refused while that lease is live. Two things must be
true of the implementation, and each has a planted bug in the model:

1. The renewal is decided against the current state, including changes whose
   barrier has not finished. A takeover applied first makes the renewal fail.
   Deciding from an older copy answers a renewal after the lease moved
   (`stale-view`).
2. No other command may write back a lease value it read before the renewal.
   The engine now reads the clock, prepares and applies each command under one
   hold of the lock (`lost-extension`).

**The store crashes after g.** Recovery finishes at some R > g. If the durable
journal holds the lease's claim, the restarted store gives that lease a
deadline of R + L on its own clock and grants nobody else before it. R is later
than g, so the wait ends after the lost grant did. Two more conditions, each
with a planted bug:

3. The claim is durable before any renewal of it is answered early. The store
   marks a lease settled when the barrier covering its claim has finished, and
   only a settled lease is extended this way. An honest tile cannot renew
   sooner, because it learns its fence from the claim's reply, which waits for
   that barrier. The flag covers a client that guesses. The model's related bug
   answers the claim itself early (`unjournaled-claim`).
4. The restart wait is a full lease (`no-restart-wait`, `short-restart-wait`).
   L - margin plus the drift term would be enough. A full L is what E16 built,
   and it is kept.

A successor's claim cannot be in the recovered journal unless the store had
already seen the old lease expire, which by the first case was after g + L.

The other paths follow from the same two cases. `BeginTakeover` either precedes
the renewal in lock order, and the renewal is refused, or follows it, and the
replacement waits out g + L. If a crash loses the takeover record, the restart
wait still applies and the replacement's claim is still checked. A transport
retry of an answered renewal gets the same grant and does not move the
deadline. Refusals are never answered early. They take the journaled path
unchanged.

The tile is untouched. It still counts a grant from its original send, drops a
reply that arrives after its old deadline, and gates every side effect on its
local deadline (`ignores-deadline` is the control for that).

## Model

`goliath-sim renewals` adds a seeded event simulation, `renewal_crash`, beside
the E1, E9 and E16 sweeps. Real time runs in nanoseconds and never waits.

- One tile and three processes. Each claims, renews on the production
  `RenewalSchedule` (period L/4, slack L/20), acts every L/200 while its
  production `LeaseKeeper` allows, and can be partitioned, die, shut down
  cleanly, lose its lease and come back. Three in four starts force a takeover
  of whoever holds the tile.
- A store with a journal, one worker, batches and a barrier per batch that
  writes a record. Replies leave after the barrier. It uses the production
  `StoreLease`, including `renew_unjournaled` and `recover`. Barrier lengths
  follow three profiles: `calm` (1 to 40 ms), `tail` (15% take 150 to 700 ms,
  the second machine's shape) and `severe` (half take 300 to 2,500 ms).
- Up to two store crashes per run, half of them placed within a lease of a
  process start. A kill keeps every written record. A power cut keeps the
  durable prefix and a random part of the unsynced tail. The store is down for
  1 ms to 1.5 leases and comes back with a new clock rate.
- Every clock runs at its own rate within 1,000 ppm. Messages take 0.05 to 3 ms,
  and one in ten up to a quarter lease.

The checker reads only the actions and the store's state. It counts actions
taken while another process would also act, actions by a fence older than one
that already acted, and actions the running store no longer counts as inside a
live lease.

Reproduce with:

```sh
just renewals 100000 docs/experiments/data/E29/renewals.csv
```

The builder ran it on an 8-vCPU boat sandbox in 212 s:

```sh
boat-home run --wait -- 'cargo build --release -p goliath-sim && target/release/goliath-sim renewals 100000 docs/experiments/data/E29/renewals.csv'
boat-home fetch docs/experiments/data/E29/renewals.csv docs/experiments/data/E29/renewals.csv
```

[The result](data/E29/renewals.csv) has SHA-256
`2475a00a931da55478ddc88465b6526cff47d1b03a00ba8ba9fe1cbf66046806`. A second run
in the same sandbox matched it byte for byte. The sweep exits nonzero if a safe
row has a failure, if a planted bug is missed at any lease length, or if a
renewing holder stops behind a barrier on the unjournaled path. It exited 0.

### Safety

Each of the 18 safe rows has 100,000 seeds and zero failures: both renewal
paths at 500, 1,000 and 2,000 ms leases under all three barrier profiles. That
is 1,800,000 runs with no overlapping owner, no stale write and no action
outside the store's lease.

The nine unjournaled rows granted 23,672,660 renewals without a journal record
across 868,440 store crashes. In 606,958 of those crashes the recovered
counter was behind the last one granted, so a granted renewal really was lost.
Holders took 1,295,254,101 checked actions.

### Planted bugs

All six are caught at every lease length. Counts are failing seeds out of
300,000 per cell, summed over the three barrier profiles.

| Planted bug | L = 500 | L = 1,000 | L = 2,000 |
| --- | ---: | ---: | ---: |
| `no-restart-wait`: recovered leases count as expired | 95,308 | 102,625 | 108,926 |
| `short-restart-wait`: the restart wait is L/2 | 27,233 | 29,624 | 31,610 |
| `stale-view`: renewal answered after a takeover moved the lease | 35,224 | 29,309 | 21,362 |
| `lost-extension`: a takeover overwrites an extension | 26,502 | 25,500 | 20,549 |
| `unjournaled-claim`: the claim skips the barrier too | 6,938 | 6,973 | 6,309 |
| `ignores-deadline`: the tile ignores its local deadline | 259,093 | 272,162 | 279,161 |

`stale-view` and `lost-extension` need a takeover to sit in a long barrier, so
the calm profile catches them in only 48 to 183 of 100,000 seeds. The CSV has
every row and the first failing seed of each. Replay one with, for example:

```sh
cargo run --release -p goliath-sim -- renewal unjournaled stale-view 1000 severe 1
```

### Availability in the model

A stall stop is a holder that had renewed at least once and then expired while
it was connected, undrained and the store was up. Only the store's queue and
barrier were slow. A late start is a holder whose claim was answered too late
to renew even once. Counts are per 100,000 seeds.

| Lease | Profile | Journaled: stall stops | Journaled: late starts | Unjournaled: stall stops | Unjournaled: late starts |
| ---: | --- | ---: | ---: | ---: | ---: |
| 500 | calm | 0 | 0 | 0 | 0 |
| 500 | tail | 119,553 | 35,934 | 0 | 2,066 |
| 500 | severe | 51,520 | 65,327 | 0 | 1,364 |
| 1,000 | calm | 0 | 0 | 0 | 0 |
| 1,000 | tail | 48,765 | 10,296 | 0 | 448 |
| 1,000 | severe | 84,874 | 93,972 | 0 | 3,253 |
| 2,000 | calm | 0 | 0 | 0 | 0 |
| 2,000 | tail | 82 | 36 | 0 | 3 |
| 2,000 | severe | 126,658 | 108,326 | 0 | 6,494 |

The journaled path reproduces the soak failure in the model: with the measured
tail and today's lease, a renewing holder is stopped by a barrier about once in
two runs. The unjournaled path has none in any row. The 2,000 ms row is option
(b): it nearly removes the stops for this tail and returns under the severe
one.

The late starts that remain are the claim's own wait. A claim still waits for
its barrier, and one answered later than L - margin - slack after its send,
850 ms at today's settings, starts a tile that expires before its first
renewal can go out.

### What the model does not cover

It is a bounded random search, not a proof. The store in the model follows the
engine's ordering but is not the engine's code. Only the lease, keeper and
schedule kernels are shared with production. There is one tile, no transport
retry, no process pause between a gate check and its side effect (E16's
standing assumption), and no player records.

## Implementation

All changes are in `goliath-model` and `goliath-store`. No tile patch is
needed: the wire protocol and the keeper are unchanged.

- `StoreLease` has a `settled` flag and `renew_unjournaled`, which extends only
  a settled lease. `new` is unsettled and `recover` is settled.
- The store's `State` sits behind one lock. The engine takes it per command
  for clock read, prepare and apply, and releases it before the journal write
  and barrier. After a successful barrier it settles every lease.
- `Engine::renewer()` returns a `Renewer`, a cloneable handle that owns no
  thread. `Renewer::renew` returns the reply to a `RenewLease` or `None`. It
  answers only a grant: the fence is current and active, the lease is live,
  settled and not draining, the counter is higher, and the journal holds no
  receipt under that request id. `None` sends the command through
  `execute_batch` unchanged, so every refusal is produced where it always was.
- The store binary's connection threads ask the `Renewer` before queueing a
  command for the worker. A granted renewal never enters the queue.
- The last unjournaled renewal per tile is remembered in memory, so a transport
  retry gets the same grant without a second extension. It is deliberately not
  stored with the journal's retry receipts. Those are a FIFO window of 9,216
  entries that journal replay rebuilds. Entries that replay cannot see would
  shift that window, and recovery refuses a journaled command that hits a
  receipt the live run had already evicted.
- After a journal failure the state is closed and nothing is answered early.
- `--renewals barrier` restores the previous behaviour: every renewal is
  journaled and answered after its barrier. The default is `--renewals
  unjournaled`.
- With `GOLIATH_STORE_TRACE` set, early renewals are written to
  `<trace>.renewals.csv` (`request_id,accepted_ns,received_ns,answered_ns`) and
  to the connection trace with a zero wake time. They no longer appear in the
  batch and request traces.
- The `fault-injection` feature gains `--test-barrier-delay-ms`, which sleeps
  before every barrier. It removes no barrier.

### On-disk format and migration

None. Journal records, snapshots and wire tags are unchanged. A journal written
before this change, with renewal records, replays as before, and a test covers
it. Renewal counters stay increasing in any snapshot and journal the store can
write, so replay never sees one go backwards. A journal written with
unjournaled renewals only has fewer records, so the older binary should open
it too. That direction was not tested.

### What changes for callers

- After a store restart the directory's `heartbeat` for a tile can be behind
  the tile's counter until its next renewal. The next renewal is accepted,
  because its counter is higher than any the store knows.
- A renewal that was answered before a restart and arrives again after it is
  no longer recognised. The store grants it as a new renewal instead of
  returning the old grant or `RenewalReplay`. That extends the store's
  deadline, never the tile's, so it can delay a takeover by at most one lease.
  A drain that was acknowledged still refuses every renewal after a restart.
- The journal carries about 16 fewer mutations a second with four tiles.
  Compaction, which runs every 1,024 mutations, comes less often and the retry
  window covers a longer time.

### Tests

Model, in `crates/goliath-model/tests/it/leases.rs`:
`only_a_settled_lease_is_renewed_without_a_journal_record`.

Simulator, in `crates/goliath-sim/tests/it.rs`:
`unjournaled_renewals_survive_store_crashes_and_every_planted_bug_is_caught`
runs 150 seeds per profile at a 1,000 ms lease on both paths and requires every
planted bug to be caught.

Store state, with injected time:

| Test | What it shows |
| --- | --- |
| `e29_unjournaled_renewal_extends_the_lease_a_takeover_then_waits_for` | An unsettled claim is not renewed early. A settled one is, with no receipt. A later takeover keeps the extended deadline, the replacement is refused until it passes, and the successor starts unsettled. |
| `e29_every_refusal_and_remembered_retry_takes_the_journaled_path` | Non-renewals, request id 0, a replayed counter, a foreign or stale fence, an id the journal remembers, an expired lease and a closed state all return `None`. A retry returns the same grant without extending, and a reused id with another payload is a conflict. |
| `e29_a_snapshot_keeps_the_counter_and_recovery_waits_a_full_lease` | Journal-only and snapshot recovery both settle the lease, wait a full lease and accept the holder's next counter. |

Engine, with real files and threads:

| Test | What it shows |
| --- | --- |
| `e29_a_renewal_is_answered_inside_a_stalled_barrier_and_the_claim_waits_for_it` | With another command's barrier held open, the state lock is free and a renewal is answered inside that barrier. On an injected clock, a replacement is refused from the end of the original claim's window until 1 ns before the renewal's ends, and granted at that end. |
| `e29_recovery_replays_a_journal_that_never_saw_the_renewals` | Journaled and unjournaled renewals interleave around a snapshot. 9,224 unjournaled renewals leave the journal's three receipts in place. The store reopens, reports the journal's last counter and grants the next one early. |
| `e29_a_failed_journal_stops_early_renewals` | After an injected barrier failure the early path answers nothing. |

Real processes, in `crates/goliath-store/tests/it/leases.rs`:

| Test | What it shows |
| --- | --- |
| `unjournaled_renewals_are_lost_by_sigkill_and_the_restart_waits_them_out` | Three renewals leave the journal's length unchanged. After SIGKILL and 500 ms down, the directory shows counter 0, a claim is still refused after the lost grant's window, the holder's next renewal is granted, a takeover drains it, and the replacement gets the tile no earlier than one lease after that last renewal. |
| `the_barrier_mode_still_journals_renewals_and_its_journal_replays_unchanged` | `--renewals barrier` grows the journal per renewal. That journal opens in the default mode with its counter intact. |
| `a_stalled_barrier_delays_a_renewal_only_in_barrier_mode` | Needs `--all-features`. With every barrier delayed 600 ms and another mutation in flight, the renewal returns in under 300 ms on the default path and takes at least 600 ms in barrier mode. |

### Gates

All on a boat fork with rustc 1.99.1, because a measurement window held the
host:

```sh
boat-home run --wait -- 'cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
boat-home run --wait -- 'cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features'
```

Formatting and both clippy runs pass with no diagnostics. `cargo test
--workspace` passes 92 tests and `--all-features` 93. `goliath-sim leases 20000`
still reproduces `data/E16/leases.csv` byte for byte. The timing tests ran on
shared sandbox vCPUs, not on either test machine.

### Flaky test

The reviewer's first gate run had one failure in the store's lib tests. Loops
on a fresh fork reproduced it in 2 of 400 runs, both in
`e29_a_renewal_is_answered_inside_a_stalled_barrier_and_the_claim_waits_for_it`.
One run hit the "host too slow to judge" assertion. In the other, the takeover
returned `LeaseExpired`. The test stalled a barrier with a 1 s sleep, read the
real clock, and left 300 ms after that barrier for a real fsync and two more
journaled commands. Under fsync load that was not enough, and the renewed
lease had really expired by the time the takeover ran. The store's behaviour
was correct in both runs. The test only exists on this branch.

The test now injects the store's clock, read under the state lock in the
engine and the `Renewer` (test builds only), and holds the other command's
barrier open on a gate instead of a sleep. It checks the exact boundaries: the
replacement is refused at the end of the original window and 1 ns before the
renewal's end, and granted at that end. It also checks that the state lock is
free while the barrier is held. Two mutations fail it: holding the state lock
across the barrier, and an early renewal that does not move the deadline.

After the fix, on boat, the lib test binary ran 1,200 times with no failure:
300 plain, 300 with `--all-features`, 200 under fsync load, 200 in concurrent
pairs, 100 pinned to one CPU and 100 with all CPUs busy. A last change only
lets a failed assertion end the test instead of hanging it. The committed code
then passed 300 more runs of the binary with and without `--all-features`, and
30 runs each of `cargo test --workspace` with and without `--all-features`.
Formatting and both clippy runs pass. The sweep does not link
`goliath-store`, so it was not rerun.

## Unproven

- Nothing was run on real tiles. Whether the soak passes on the second machine
  is the open question.
- The store process can stall for reasons other than fsync. A renewal now waits
  for the state lock, which the engine holds while it applies a command and
  while it serializes a snapshot. Neither does IO, and neither hold time was
  measured under load. Scheduling delay on the server cores, swap-in and trace
  writes to the slow volume can each still delay a reply.
- Claims, takeovers, releases and player commits still wait for the barrier.
  A hand-over commit still sees a 460 ms stall, and a claim answered more than
  850 ms after its send starts a tile that cannot renew. The model counts 448
  such late starts per 100,000 runs under the measured tail. Startup
  `Directory` and `CheckLease` calls queue behind a barrier in flight too.
- Power loss was not tested on hardware. The SIGKILL tests prove process
  recovery, and the model covers a lost unsynced suffix.
- A restart from an older copy of the store directory, or on a host whose
  clock rate is outside the bound, is outside the argument.
- The M3 crash campaign and the E13 reproductions were not rerun with this
  store.
- The harness scripts that watch store mutation rates and traces were not run
  against the new trace shape. No script reads `renewals.csv` yet.

## What the reviewer must measure

1. Reproduce the gates and the sweep with the commands above. The CSV must
   match the hash.
2. Build the store from this branch and freeze it beside the unchanged
   `final-r2` tiles. Only `goliath-store` differs.
3. Run the 30-minute soak on the second machine with the `e28-soak-r2` fixture.
   It passes with no authority stop and no clock anomaly on any tile, while
   the store's batch trace shows the barrier tail the earlier runs had. A run
   whose largest barrier stays under about 300 ms does not test the change.
   Report the largest `answered_ns - received_ns` and the largest
   `received_ns - accepted_ns` from `renewals.csv` beside the largest barrier.
4. Run a control that shows the change is the cause. Either the same binary
   with `--renewals barrier`, which needs one argument added to the cluster
   launcher, or a short directed cell with a `fault-injection` build and
   `--test-barrier-delay-ms 700` in both modes. The delayed build is a
   diagnostic and never a measured binary. Tiles should keep their leases in
   the default mode and lose them in barrier mode.
5. Kill and restart the store under load. Tiles that reach the restarted
   store inside their window should carry on, any replacement should wait a
   full lease from recovery, and the ownership audits should show no overlap.
6. Rerun the M3 crash campaign, since the store's lease behaviour changed.
7. Rerun the E15 and E16 64-client latency rows. The commit P95 line of 20 ms
   must hold with renewals on the early path and the state behind a lock.
8. Check the store's memory trace and the mutation-rate checks against the
   lower mutation rate.

## Proposed decision

Proposed text for `docs/decisions.md`. The reviewer decides.

> ## D13. Lease renewals don't wait for the journal
>
> A tile's lease renewal is answered from the store's memory and is not
> journaled. Claims, takeovers, releases and every player commit still wait for
> the durable barrier. Lease length, margin and cadence stay at 1,000 ms,
> 100 ms and 250 ms.
>
> The reason: with journaled renewals a tile's authority depended on the worst
> fsync of the store's volume. On the second machine a 460 ms barrier followed
> by a 180 ms one outlasted the 600 ms renewal budget and stopped two tiles
> (E28). A longer lease only moves that line and doubles failover time.
>
> It is safe because a restarted store already waits a full lease before it
> grants any recovered tile to someone else (E16). A renewal lost in a crash
> ended before that wait does. Three rules carry the argument, and E29's model
> catches the removal of each: the store waits a full lease after every
> restart; a renewal is decided against the same lease state that claims and
> takeovers read and write; and a lease is renewed this way only once its claim
> is durable. E29 found zero failures in 900,000 modelled runs of this design
> with store kills and power cuts.
>
> This holds only while the tile obeys its local deadline, clocks stay within
> their rate bounds, and the store restarts from the directory it last wrote.
> The renewal counter no longer survives a store restart. The decision stands
> once the reviewer's soak on the second machine passes with this store.
