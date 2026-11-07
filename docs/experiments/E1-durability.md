# E1: durability at a tile wall

## Decision

Start with P2, a coherent source-tile checkpoint before the player generation CAS.
It had zero item duplication, identity loss, ownership violations and stale writes
in the fault sweep. P0 and P1 failed on the same seeds. This is evidence about this
model, not a proof about Horizon.

P2 only meets the crossing target if checkpoints are cheap enough. With the assumed
8 ms fsync and 3 ms one-way control delivery, checkpoint work must take **less than
128 ms** to keep the fault-free input gap below 150 ms. The real checkpoint cost is
unknown. Measure it before adopting this policy in a real server.

P4 also had zero duplication and loss, with zero authoritative rollback. It is the
fallback if P2 cannot fit its latency budget. Its durable-before-ack tick batches
move cost onto normal actions. P3 makes the first route fast but requires a large
rollback domain and an embargo on further crossings. I would not start with it.

## Scope and assumptions

This is a standalone T0 experiment. There is no Horizon dependency, real disk IO,
network IO, async runtime or wall clock in a scenario. There are three square tiles
in a horizontal line, three players, eight unique item identities, one abstract
container per tile and dropped items. Counts represent copies at each location,
so two copies inside one tile are visible too. Item identities never intentionally
appear or disappear. There is no crafting, consumption, stacking or item creation.

Each seed has the same scripted chest and donor/drop/pickup chain, followed by 28
random bot commands. Policies receive the same input and fault schedules, but
frozen players cannot execute commands. P4 queues inputs behind its tile's pending
durable batch. P3 rejects onward crossings during speculation. These differences
are counted, not treated as equivalent completed work.

All latency numbers below are assumptions, in milliseconds:

| Setting | Checkpoint work | Fsync | Checkpoint interval | One-way control delivery |
| --- | ---: | ---: | ---: | ---: |
| 0 | 10 | 2 | 100 | 3 |
| 1 | 80 | 8 | 200 | 3 |
| 2 | 250 | 20 | 400 | 3 |

A checkpoint takes checkpoint work plus one fsync. The store CAS takes another
fsync. IO completions are atomic. A crash before completion leaves the old durable
state. This assumes a correct write/fsync/atomic-install adapter and durable media
that do not lose acknowledged writes. Checkpoints and the shared store are separate
durability domains, not an imaginary transaction covering both.

Fault schedules include tile crashes, including during pending IO, partitions of
tile/store, gateway/tile and tile/tile links, store unavailability, and a message
chaos interval. Chaos loses some deliveries, retries with seeded 15 to 37 ms delays,
and duplicates commit/arrival messages. Those delays and overlapping messages
produce reordering. Faults heal by virtual 800 ms. Execution stops at 2,000 ms,
then every tile is recovered again to expose dormant bad saves. Recovery cannot
serve inputs before its store read succeeds. The invariant checker can inspect
the logical recovered state, including committed escrow, even during a partition.

## Policies actually implemented

P0 keeps independent source and target player saves. The target saves its imported
inventory and starts before the source release. There is no CAS or fence in this
path. A source checkpoint can later restore its old player copy. This models the
unsafe independent-save contract, not Meridian's code or its wire timings.

P1 adds a linearizable player generation CAS and source/target process fences.
Store entry inventories are recovery anchors, not additional live inventories.
Only a resident with the store's owner and generation is authoritative. If routing
has not installed a committed entry, that entry is authoritative escrow. This fixes
the independent player-owner problem but cannot stop a chest or donor rollback.

P2 freezes the departing player and snapshots the entire coherent source tile.
That snapshot includes containers, drops and every resident inventory, not only
the departing player. It must become durable before CAS. Recovery filters departed
generations, uses checkpoint state for matching residents, and materializes newer
committed entries when needed. It never replaces a checkpointed matching player's
inventory with an older store entry. If the source dies before its checkpoint,
the handover aborts. If it dies after CAS, the target's durable entry wins.

P3 takes the same source snapshot but routes a tentative player before it lands.
The model keeps a volatile undo image for the whole three-tile cluster. Target
deposits, drops, pickups and effects on other players are part of that branch.
All new handovers and new background checkpoints are embargoed until the source
checkpoint and store commit finish. A crash of any tile aborts the branch, restores
all its live images and the original store record, then recovers the failed tile.
Already submitted checkpoints contain only pre-branch state. The tentative record
is a routing view; it is not a durable store commit. This is deliberately broader
than causal dependency tracking. The branch cannot escape to an untracked tile.

P4 makes an atomic durable delta batch before acknowledging an item operation.
The virtual tick is 50 ms. This small implementation serializes one pending input
batch per tile rather than packing many commands into one batch. It journals target
arrival before the target acts, and anchors a recovery-materialized entry before
any dependent delta. Checkpoints compact only the durable log head. A late
checkpoint must retain newer log records. Old-epoch completions cannot land.
Recovery replays durable deltas and discards unacknowledged pending operations.

Fencing assumes a linearizable ownership directory with one current process token
per tile. The simulator tests delayed requests from replaced epochs, not competing
implementations of directory consensus. No tile can serve as a second authorized
owner. The planted mutation disables that fence and the independent stale-write
checker fails as expected.

## Measurements

Run on 2026-11-07, Rust 1.98.1, local Apple M5 Pro. The sweep used 18 independent
worker processes, the available core count. Each scenario has one scheduler and no
threads. Seeds 0 through 66,666 run for each policy and setting. That is 66,667 per
row, 200,001 per policy and **1,000,005 scenario-seeds total**. All three settings
exposed P0 and P1 violations in the same sweep that accepted P2 through P4.

Raw results are in [data/E1-results.csv](data/E1-results.csv).

| Policy | Assumed checkpoint/fsync | Failing seeds | Duplicate identities | Loss incidents | Rollback records | Gap P50 | Gap P95 | Gap P99 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| P0 | 10/2 | 65,650 | 203,975 | 10,060 | 118,039 | 8 | 459 | 555 |
| P1 | 10/2 | 24,572 | 46,979 | 310 | 71,554 | 8 | 458 | 555 |
| P2 | 10/2 | 0 | 0 | 0 | 24,210 | 20 | 472 | 576 |
| P3 | 10/2 | 0 | 0 | 0 | 73,242 | 3 | 400 | 518 |
| P4 | 10/2 | 0 | 0 | 0 | 0 | 147 | 540 | 641 |
| P0 | 80/8 | 65,099 | 196,822 | 9,738 | 287,269 | 14 | 468 | 568 |
| P1 | 80/8 | 53,815 | 102,909 | 1,590 | 163,629 | 14 | 465 | 567 |
| P2 | 80/8 | 0 | 0 | 0 | 80,946 | 102 | 508 | 624 |
| P3 | 80/8 | 0 | 0 | 0 | 221,006 | 3 | 311 | 491 |
| P4 | 80/8 | 0 | 0 | 0 | 0 | 157 | 551 | 647 |
| P0 | 250/20 | 64,038 | 185,889 | 7,767 | 443,140 | 26 | 480 | 590 |
| P1 | 250/20 | 56,389 | 109,829 | 4,330 | 245,003 | 26 | 478 | 587 |
| P2 | 250/20 | 0 | 0 | 0 | 183,588 | 203 | 489 | 584 |
| P3 | 250/20 | 0 | 0 | 0 | 401,495 | 3 | 231 | 426 |
| P4 | 250/20 | 0 | 0 | 0 | 0 | 176 | 571 | 661 |

Duplicate identities count each duplicated identity once per scenario, even if it
duplicates repeatedly. Loss incidents count transitions from present to absent.
An identity restored later does not erase its earlier loss incident. The CSV
attributes each incident to crash/final recovery or to another protocol event.
All 6,230 P1 loss incidents happened during recovery. P0 had 2,120 recovery losses
and 25,445 other losses. Unsafe independent inventory replacement accounts for the
second class. P2 through P4 had zero in both classes.

Rollback records are the sum of changed authoritative item-location rows and
changed player-owner rows at each rollback. They count lost progress, not destroyed
identities or bytes. Restoring a dropped item to its earlier inventory is rollback,
not item loss. Committed escrow becoming resident and stale replica cleanup do not
count. P4's zero is about acknowledged state, not pending unacknowledged inputs.

Gap measures a started crossing request to first target actionability or abort.
For P4 it includes time queued behind a pending tick batch. For P3 the first
actionability is tentative and can be revoked. Aborted crossings contribute their
freeze duration. This is not a total service-outage percentile: recovery downtime
after an abort is separate, and rejected crossing requests have no gap sample.
The CSV includes routed crossings, aborts, rejected inputs, P3 embargo rejections
and right-censored handovers. There were no right-censored handovers in this run.
Routing and abort counts overlap if a tentative P3 route later aborts.

Do not read the lower P3 P95 at higher checkpoint latency as an improvement.
Longer speculation rejects more crossings and changes the mix of sampled events.
In setting 1 alone, P3 rejected 151,727 crossing commands under its embargo.

### Fault-free crossing budget

The extra calm run used 1,000 seeds per policy and setting, 15,000 scenarios, with
the same bot inputs and no injected faults. It still performs the final recovery
check. Full results are in [data/E1-calm.csv](data/E1-calm.csv).

| Policy | Gap P50/P95/P99, 10/2 | Gap P50/P95/P99, 80/8 | Gap P50/P95/P99, 250/20 |
| --- | ---: | ---: | ---: |
| P0 | 8/8/8 | 14/14/14 | 26/26/26 |
| P1 | 8/8/8 | 14/14/14 | 26/26/26 |
| P2 | 20/20/20 | 102/102/102 | 296/296/296 |
| P3 | 3/3/3 | 3/3/3 | 3/3/3 |
| P4 | 10/156/156 | 22/174/174 | 46/210/210 |

P4's normal accepted-action P95 waits were 148, 154 and 166 ms. These include
queueing behind earlier bot commands, not only fsync. The one-batch-per-tile
implementation is conservative and not a throughput prediction for a real grouped
tick log. P0 and P1's short gaps are unsafe, not acceptable optimizations.

For P2 without faults or tick-alignment work outside the model:

```text
gap = checkpoint work + 2 * fsync + 2 * one-way control delivery
```

With 3 ms delivery, checkpoint work must be below 140, 128 or 104 ms for fsync
values 2, 8 or 20 ms respectively. These are model budgets, not real measurements.
If checkpoint latency includes its own fsync already, its complete durable latency
must be below 142, 136 or 124 ms respectively. Real tick alignment, queueing,
gateway work and client processing must come out of the same 150 ms budget.
For variable distributions, measure the combined P95 directly. Adding independently
measured component P95s does not establish an end-to-end P95 guarantee.

No policy can guarantee a 150 ms input gap during an arbitrarily long partition.
The fault sweep deliberately does not meet that latency target.

## Counterexamples and replay

```sh
just replay P0 0 0
just replay P1 6 0
```

P0 seed 0 has original trace hash `bb7a323ac82c0423`. Its reduced duplicate witness
has hash `dfb991f2f3871d4d`. Player 1 takes item 0, crosses, then source and target
both own that inventory at target activation. No crash is needed. The focused test
`p0_duplicates_with_one_crossing_and_no_faults` is shorter still: initially equipped
player 0 crosses once. Removing that only input removes the failure.

P1 seed 6 has original hash `493431a8a1666ee6`. Its reduced hash is
`d5f674e5e5771983`:

```text
 0 ms  player 1 takes item 0 from A's chest
 6 ms  player 1 freezes and requests A -> B
11 ms  generation CAS commits B
14 ms  B starts the player
17 ms  A releases the old player
57 ms  A crashes, restoring item 0 in the chest
       B still holds item 0. The checker counts two copies.
```

Neither a stale player write nor a second player owner is needed for P1's failure.
The donor case is tested separately: player 0 gives item 4 to player 1, player 1
crosses, and player 0's old inventory returns after A's crash. A coherent checkpoint
of every source resident closes this path too.

Replay always regenerates the original seeded run before shrinking. The shrinker
deletes faults and then bot inputs, preserving duplication if the original run
duplicated. Otherwise it preserves the original ownership or stale-write failure
class. It stops when no single deletion preserves that failure. This is 1-minimal
under deletion, not a globally shortest trace over all possible timings. It prints
the remaining schedule and the event prefix through the first invariant failure.
The original hash and reduced hash include the entire run, including final recovery.

## Verification and failures encountered

Commands run successfully for the delivered state:

```sh
cargo run --release -p goliath-sim -- sweep 66667 docs/experiments/data/E1-results.csv
target/release/goliath-sim calm 1000 docs/experiments/data/E1-calm.csv
just replay P0 0 0
just replay P1 6 0
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
/usr/bin/time -p cargo test --workspace
```

The final timed workspace test run passed in 1.27 seconds wall time, including Cargo and
doctests, well below 30 seconds. The architecture audit found no missing module
ownership docs, lint suppressions, unsafe blocks, wildcard imports or kind-named
modules. Both crates inherit the requested workspace lints. There are no external
Cargo dependencies.

Root `target/` reached 75 MB. After verification, `scripts/clean-builds --all`
removed that disposable output and freed 75 MB. No compiled binary is retained;
the replay and sweep recipes rebuild it. No commit was made.

Result-file SHA-256 values:

```text
E1-results.csv e70e7e27b0bdd11566b590166a137a59df3b71d3f250172f7cd0de68150c66b7
E1-calm.csv    b09f4d0176d605c468cdfa723f143c26d254dc7f0e2e2358a81e579a97a5ff80
```

The workspace has 17 focused tests. They cover deterministic hashes and traces,
fencing disabled on purpose, generation conflicts/idempotency, same-tile copies,
negative coordinates, stale checkpoint completion, log compaction, donor rollback,
replay shrinking, store-unavailable recovery, and millisecond crash cuts across a
handover/checkpoint. P4 also runs 1,000 seeded slow-disk regression scenarios in the
normal test suite. No million-scenario sweep runs inside `cargo test`.

During implementation the smoke sweep rejected early P4 versions. The first missed
the durable arrival baseline. Another allowed a checkpoint of unlogged lifecycle
state to become the wrong base for later log deltas. Those were model bugs, fixed
with specific regression tests. Initial clippy found a manual divisibility check;
it now uses the standard method, without lint suppression. A first calm run was
incorrectly held to the fault-sweep control gate; the calm command now skips that
gate, while the main sweep still requires both P0 and P1 to fail in every setting.

## What this does not establish

- This is bounded deterministic fault testing, not exhaustive state-space checking.
  It does not prove safety for unbounded players, items or schedules.
- The directory and player store are linearizable and durable by assumption. Store
  consensus, torn records, disk corruption, disk loss and Byzantine processes are
  outside the model. Only tile processes are killed. Store failure is unavailability,
  and gateway failure is represented by connectivity faults, not a proxy restart.
- Checkpoint installation and each durable batch are atomic. Their real encoding,
  fsync/rename implementation and crash behavior still need testing.
- A checkpoint is a coherent tile-wide cut. Independent Anvil, player and entity
  saves do not automatically have that property. P2 cannot claim safety until
  Horizon implements and verifies that cut and its generation-aware recovery.
- P3's whole-cluster undo memory is volatile. It models a recovery policy that can
  coordinate surviving processes, not a persisted distributed undo implementation.
  A coordinator crash during speculation is not tested. It cannot safely publish
  external effects or cross beyond its embargo without extending the rollback domain.
- There are no mobs, hoppers, world generation, nested containers, crafting, deaths,
  client acknowledgments, terrain packets or Minecraft-specific inventory rules.
- The gap samples measure modeled handovers, not client-visible motion or total
  availability. Bot rejection counts must accompany any throughput comparison.
- Latencies have no measured variance, IO contention or checkpoint-size cost. The
  model does not predict CPU scaling, disk bandwidth or a real server's P95.

## What to measure on Horizon

1. The durable time for a coherent source cut containing chunks, containers, dropped
   items and every resident player at one freeze tick. Include first-call and loaded
   cases, the fsync and atomic install, and checkpoint size under realistic load.
2. Kill the engine before and after each write, fsync, install, CAS and routing
   acknowledgment. Check identities in chests, donor players, targets and onward
   crossings after recovery. An ordinary graceful stop is not this test.
3. Verify stale process generations cannot write player records or install tile
   checkpoints, including requests queued before a partition and delivered later.
4. Measure moving-player input acknowledgment across the wall, including tick
   alignment, gateway queues and client processing. Compare its end-to-end P95
   with the 150 ms budget, not only the export request's round trip.
5. If P2 exceeds its budget, prototype a real grouped durable tick log. Measure its
   fsync distribution, batching throughput, arrival anchors, compaction and action
   wait. P4's identity safety is promising; this simulator's serial queue is not a
   production performance design.

Implementation used `docs/decisions.md`, the permitted Horizon research report and
Meridian's player-state/fencing analysis. No Folia report, Paper source or other
GPL source was read or copied. The model is written from first principles.

## Independent verification

The reviewer re-ran this experiment on 2026-11-07, separately from the agent that built it.

- `cargo fmt`, `cargo clippy -D warnings` and `just test` passed. 17 tests, 0 failed.
- `just sweep` reproduced `E1-results.csv` byte for byte, SHA-256 `e70e7e27...`, in 5.3 s.
- `just replay P1 6 0` printed the trace above and the same hashes.
- The rust-project-architecture audit reported 0 undocumented modules, 0 unsafe blocks and 0 lint suppressions, and both crates inherit the workspace lints.

The zero rows only count if the checker can see the bugs that matter, so P2 was mutated in a scratch copy and swept at 20,000 seeds per setting. Every planted bug was caught.

| Mutation of P2 | Failing seeds at 10/2, 80/8, 250/20 | Result |
| --- | ---: | --- |
| CAS commits before the checkpoint is durable | 611, 5,867, 14,349 | Caught: duplicates |
| World and player inventories saved separately | 16,288, 11,677, 3,509 | Caught: duplicates and losses |
| Recovery prefers the older store entry | 17,875, 12,521, 3,514 | Caught: duplicates and losses |
| The handover's checkpoint is never installed | 6,208, 7,812, 1,080 | Caught: duplicates |
| Crash aborts arrive 40 ms late, fencing on | 0, 0, 0 | Safe |
| Crash aborts arrive 40 ms late, fencing off | 2,326, 2,770, 1,468 | Caught: stale writes |

The last two rows close a gap in the model. The simulator aborts a crashed tile's uncommitted handovers at once, which is perfect failure detection that a real system doesn't have. With that delayed, stale CAS requests do reach the store. Fencing rejects them, and removing fencing lets them through.

Two consequences for real Horizon:

- Separate world and player saves fail, so Horizon's current saves don't meet P2. M1 needs one coherent tile-wide cut.
- A real store needs the fence check on every CAS. It is load-bearing, not a backstop.
