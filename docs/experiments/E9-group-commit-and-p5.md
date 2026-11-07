# E9: group commit and log-structured cuts

## Result

P5 passed the safety criterion. The final sweep ran 1,200,006 scenario-seeds,
including 200,001 P5 scenarios. P5 had zero duplicate identities, loss incidents,
owner violations and stale writes. P0 and P1 failed in every setting.

Group commit is implemented and passed the final 1,000-kill store experiment.
The latency result is qualified, not an unconditional pass. At 64 clients with
`F_FULLFSYNC`, the independent Rust driver measured P95 18.233 ms. The original
Python driver measured 35.398 ms, above the 20 ms pass line. Both results are kept.
Do not tick the M2 store milestone from the faster driver alone.

Applying 40 ms crash-abort notifications to every policy also exposed P3's
dependence on immediate cluster rollback. P3 now fails. The sweep deliberately
returns exit status 1 for that finding, while retaining all rows. P2, P4 and P5
remain safe. No workload or fault schedule was changed to remove failures.

Implementation is complete, pending review. Nothing was committed,
pushed or sent to GitHub. No `.local/src/` source was changed. No Folia, Paper,
GPL source or prohibited research report was read.

## Store ordering and ownership

`Engine::execute_batch` validates each command against the private state after
its predecessors. Successful mutations and their receipts get consecutive
journal records. One contiguous append buffer and one selected barrier cover
the batch. Reads, retries and application-fault replies also wait for that
barrier before leaving the engine. IO failure returns no batch responses and
closes the whole engine, including reads.

The binary owns a bounded 1,024-request queue and the batching worker. It collects
for at most 500 microseconds, with a 256-request cap. A pool of 64 connection
threads replaces per-request thread creation. A separate 512-socket admission
queue lets 256 clients wait without requiring 256 active connection workers.
Each connection still carries one request and response. Full admission closes
the new socket without accepting the command. Existing retry semantics apply.

Automatic compaction runs between batch replies. Replies already covered by the
journal barrier do not wait for compaction's additional barriers. Explicit
`Compact` still completes before its reply. A compaction IO error closes the
engine. Snapshot installation, journal prefix checks, retry receipts, the stable
writer lock, source and target fences, and directory barriers retain E5's rules.

The persistence library owns no threads. `Engine::maintain` is caller-driven.
The new load driver is a binary, not a runtime hidden in library code. There are
no new crates or external dependencies. The Rust architecture skill kept cut-log
storage in the model and scheduling in the simulator.

## Store crash and fault evidence

The final `just store-torture` run used the release binary with SHA-256
`7a41b4bc44a4f79f3821f0b03a0fa370a67cc210d923360c89ea6da3c8bb5570`.
It completed in 109.240 seconds. Full evidence is in
[data/E9/all-results.json](data/E9/all-results.json).

| Seed | Kills | Clients | Load acknowledgements | Unknown calls | Missing commands completed by retry | Snapshot versions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 5001 | 500 | 8 | 6,542 | 3,995 | 2,366 | 159 |
| 5002 | 500 | 8 | 7,343 | 4,000 | 2,302 | 172 |

There were zero lost acknowledgements, backward generations and owner violations.
The surviving observer checks recovered state before replaying uncertain commands.
It checks deterministic blob contents, ownership, directory tokens and the original
request ID and payload. Automatic compaction remains enabled during the kills.

| Check | Result |
| --- | --- |
| Delayed stale CAS, source and target replacement | 32/32 rejected |
| Same check with fencing disabled | 32/32 stale CAS accepted, assertion failed as required |
| Truncate the 154-byte final record at every incomplete offset | 154/154 passed |
| Flip one bit at every byte of that record | 154/154 passed |
| Tail-test process starts, including the valid complete-record control | 617/617 passed |
| Same-batch claims, stale fences, reads, duplicate receipts and conflicting payloads | Passed |
| Multiple same-batch mutations use exactly one journal barrier | Passed |
| Inject a barrier IO failure | No replies, subsequent reads fail closed |

The short suite also reruns E5's real-process CAS and claim races, dropped-response
retry, exclusive writer, snapshot-installed/log-not-truncated recovery, incomplete
next snapshot, bad snapshot and interior-corruption checks.

### Early acknowledgement control

Both modes use the same test-only file-cache layer. Journal writes first enter a
volatile byte buffer. A correct barrier writes the buffer to the real journal and
executes the selected disk barrier. The mutation skips the commit batch's barrier
and returns its response. Killing the process drops that buffer.

| Mode | Acknowledged generation | Recovered generation | Same lost-acknowledgement assertion |
| --- | ---: | ---: | --- |
| Barrier before acknowledgement | 1 | 1 | Pass |
| Acknowledge before barrier | 1 | 0 | Fail as required |

The failure is `lost acknowledgement after unflushed cache loss`. The normal
build accepts neither `--test-power-loss-layer` nor `--fault-ack-before-barrier`.
These controls require `fault-injection` and disable automatic compaction in the
control fixture, so a later snapshot cannot cover the deliberately omitted barrier.

SIGKILL alone cannot detect this bug. The kernel retains ordinary file writes
after process death, so an unsynced write may survive every process-kill trial.
The volatile file layer supplies a controlled loss of unflushed writes. This
checks ordering and checker sensitivity, not actual power-loss behavior of this
Mac's controller and SSD. The real store still issues `F_FULLFSYNC`, with no
silent weaker fallback.

## Store latency

Measured on the shared Apple M5 Pro, macOS 26.5.2 arm64, Rust 1.98.1. Load average
is recorded before and after each row. The final Python full-barrier rows began
at 30.81/25.95/17.49 over 1/5/15 minutes. The Rust 64-client full-barrier row began
at 30.03/24.59/15.49. A host sample during this work showed 1.42% CPU idle.
Other agents were running experiments. This was not an isolated timing run.

Each client has one outstanding commit and its own player. Every timed call opens
a fresh TCP connection, alternates owner, sends a 256-byte blob and checks the
generation response. Compaction runs every 256 mutations. Percentiles are nearest
rank across every timed sample, without trimming. Samples per client are 256, 64,
16 and 8 at 1, 8, 64 and 256 clients respectively. The short latency sets limit
disk interference but do not establish a long-run tail distribution.

| Barrier | Clients | Samples | Python P50/P95/P99 ms | Rust P50/P95/P99 ms |
| --- | ---: | ---: | ---: | ---: |
| `fsync` | 1 | 256 | 1.017/3.010/3.990 | 0.994/1.067/1.155 |
| `fsync` | 8 | 512 | 1.401/2.089/2.432 | 1.150/1.390/2.456 |
| `fsync` | 64 | 1,024 | 10.243/18.996/24.088 | 2.880/4.365/5.137 |
| `fsync` | 256 | 2,048 | 42.859/73.920/90.997 | 5.352/41.139/70.567 |
| `F_FULLFSYNC` | 1 | 256 | 4.038/6.994/12.980 | 4.011/6.162/10.964 |
| `F_FULLFSYNC` | 8 | 512 | 5.439/12.672/33.387 | 4.977/8.128/24.068 |
| `F_FULLFSYNC` | 64 | 1,024 | 13.655/35.398/41.655 | 9.802/18.233/21.934 |
| `F_FULLFSYNC` | 256 | 2,048 | 38.457/58.115/67.940 | 21.997/109.847/114.523 |

The Rust driver avoids Python's interpreter lock and uses the Rust blocking client
with one attempt. Its timer includes request encoding, connection, send, read and
response checking. Both drivers construct blob contents before the timer. Python
warms clients concurrently; Rust warms them serially before the synchronized timed
start. Rust uses player/generation-prefixed fixed bytes instead of Python's digest
pattern. It verifies final owner, generation and blob for every player. Neither
driver reuses a connection. Different drivers and shared load prevent attributing
the entire difference to Python alone.

The original E5 driver's pass line is still missed. The independent Rust result
shows a sub-20 ms result is possible with the grouped service, not that every
client or shared-load run meets it. A quiet repeat with more samples and the
reviewer's chosen acceptance driver is still needed. The 256-client rows do not
support a scale or latency guarantee.

Early measurements are retained too. Before maintenance moved after replies,
Python full-barrier P95 at 64 clients was 26.211 ms. The intermediate full run
measured 32.258 ms, and the pooled-service smoke run measured 28.295 ms. These are
not comparable isolated A/B trials. See `initial-latency-results.json`,
`intermediate-all-results.json` and `latency-results.json` under `data/E9/`.
Final Python samples are `latency.csv`; independent Rust samples and metadata are
under `data/E9/rust-driver/`.

## P5 model

P5 captures one coherent image at the freeze tick. That immutable model image
stands for dirty-state references, not a measured deep copy or serializer. Each
cut has a `CutId` and fence. Its delta is relative to the preceding captured
image, including every dirty chest, drop and resident inventory. Normal item
actions change volatile state immediately. They have no per-action durability
acknowledgement. A crash may roll them back to the last durable cut.

Workers append cuts after seeded variable delays. Physical append order may
differ from capture order. Each tile has one in-flight barrier. Its submitted
physical endpoint is fixed when it starts; later appends can share the next
barrier. Publication advances only through consecutive capture IDs whose records
are covered by a completed barrier. A missing earlier delta blocks later dependent
cuts. A crossing waits for its own cut, not every newer cut or background job.

Periodic background captures use E1's existing intervals. Compaction captures the
durable head and installs it later, concurrently with appends and barriers. It
retains newer records, including physically reordered records. Physical append
positions remain stable across compaction so a pending barrier cannot accidentally
cover a later append. Epoch checks reject old compaction and barrier completions;
full fence checks reject stale appends.

Recovery installs a base image and replays the valid consecutive cut prefix. The
crash model loses the unflushed suffix, including an interrupted frame and valid
cached successors after it. It never replays beyond a torn record. Store-based
generation filtering and incoming-player materialization retain P2's rules.
This is an abstract record-validity model, not a second real on-disk tile codec.
E5/E9's store tests supply the separate every-byte torn-record evidence.

Focused tests cover reverse worker completion, two cuts sharing a flush, a newer
cut outside that flush's coverage, compaction prepared before a later durable cut,
torn suffix loss and stale worker rejection. P5 also joins the millisecond crash
cuts through crossings. The existing scripted and random workload is unchanged.

## Same-seed sweeps and delayed aborts

Seeds 0 through 66,666 run for six policies and three latency settings, or
1,200,006 scenarios per full sweep. Latencies remain E1's assumed 10/2, 80/8 and
250/20 ms checkpoint/fsync pairs. These are not Horizon performance measurements.
Each scenario is single-threaded; the sweep uses 18 independent worker processes.

`instant-abort.csv` prints P5 beside the old policies with abort delay zero.
Removing its three P5 rows gives E1's original CSV byte for byte. The original
E1 CSV was not edited. Its SHA-256 remains
`e70e7e27b0bdd11566b590166a137a59df3b71d3f250172f7cd0de68150c66b7`.
The P1 seed-6 original trace hash is also a permanent regression test.

`Settings::default()` now uses 40 ms. This applies to all crash notifications,
including P3's speculative branch rollback. An explicit zero setting exists for
historical reproduction, not as the normal safety setting. Immediate store
rejection of a stale CAS may still abort a flight before its crash notification.

Final default-setting results are in [data/E9/sweep.csv](data/E9/sweep.csv).
Each row has 66,667 seeds. Owner violations and stale writes are zero for P1
through P5. P5 has zero losses in all three rows, not merely zero duplicates.

| Policy | Checkpoint/fsync | Failing seeds | Duplicate identities | Loss incidents | Rollback rows | Gap P50/P95/P99 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| P0 | 10/2 | 65,747 | 206,088 | 12,329 | 118,155 | 8/462/557 |
| P1 | 10/2 | 24,571 | 46,978 | 310 | 71,503 | 8/460/555 |
| P2 | 10/2 | 0 | 0 | 0 | 24,115 | 20/473/576 |
| P3 | 10/2 | 3,217 | 6,035 | 116 | 83,298 | 3/406/520 |
| P4 | 10/2 | 0 | 0 | 0 | 0 | 151/541/641 |
| P5 | 10/2 | 0 | 0 | 0 | 20,490 | 19/464/560 |
| P0 | 80/8 | 65,319 | 201,489 | 12,953 | 290,025 | 14/472/573 |
| P1 | 80/8 | 53,813 | 102,905 | 1,590 | 163,564 | 14/467/567 |
| P2 | 80/8 | 0 | 0 | 0 | 79,178 | 104/508/624 |
| P3 | 80/8 | 21,903 | 42,217 | 1,203 | 302,699 | 3/334/495 |
| P4 | 80/8 | 0 | 0 | 0 | 0 | 163/552/647 |
| P5 | 80/8 | 0 | 0 | 0 | 57,630 | 86/495/606 |
| P0 | 250/20 | 64,479 | 194,970 | 13,042 | 450,754 | 26/486/597 |
| P1 | 250/20 | 56,376 | 109,806 | 4,320 | 244,677 | 26/479/587 |
| P2 | 250/20 | 0 | 0 | 0 | 182,328 | 242/490/584 |
| P3 | 250/20 | 47,948 | 96,073 | 2,023 | 555,538 | 3/237/409 |
| P4 | 250/20 | 0 | 0 | 0 | 0 | 187/571/661 |
| P5 | 250/20 | 0 | 0 | 0 | 134,397 | 175/501/614 |

Failing-seed counts for historical policies retain E1's duplicate/owner/stale
definition. Loss incidents are checked separately. P5's gate includes losses too.
Raw CSVs retain completed/aborted crossings, rejected inputs and embargo counts.
No comparison claims equal completed work. No handovers were right-censored.

Compared with instant aborts, P2 stays safe but its gap P50 changes from 102 to
104 ms at setting 1 and 203 to 242 ms at setting 2. P4 stays safe, with gap P50
147/157/176 becoming 151/163/187 ms. P5's gap P50 changes from 19/79/153 to
19/86/175 ms. P0 and P1 still fail; their complete changed counts are in the two
CSVs. P3's zero rows no longer hold. D6 already rejected P3, and this adds a reason
not to treat its modelled instant actionability as a safe production shortcut.

### P3 counterexample

`target/release/goliath-sim replay P3 33 0` prints a three-event, deletion-minimal
schedule. Its reduced hash is `9762ffd904c8e02f`.

```text
 0 ms  Player 1 takes item 0 from A's chest.
 6 ms  Player 1 freezes and requests A -> B.
 9 ms  P3 routes the tentative player to B before the source cut is durable.
18 ms  A crashes just before that cut can install.
       A's recovered chest contains item 0; B still holds its tentative copy.
       The delayed cluster abort has not arrived. Two copies are counted.
```

The three external events are take, cross and crash. The route is a scheduled
protocol event. This is 1-minimal under deleting external events, not a globally
shortest schedule over arbitrary timings. The same fixture passes with immediate
abort and fails with the normal 40 ms delay. Full replay is `data/E9/p3-replay.txt`.
P5 had no failing seed to shrink.

## Permanent mutation controls

The normal short suite must catch every mutation. The longer command runs 20,000
seeds per setting per mutation, 540,000 additional scenarios. Every mutation was
caught by item conservation or the independent stale-write checker. Counts below
include losses, unlike E1's narrower historical failing-seed counter.

| Mutation | Failing seeds at 10/2, 80/8, 250/20 |
| --- | ---: |
| P2 CAS before checkpoint barrier | 619 / 6,075 / 14,621 |
| P2 split world and resident saves | 19,195 / 17,154 / 8,351 |
| P2 recovery prefers the older store entry | 19,195 / 17,154 / 8,351 |
| P2 handover checkpoint not installed | 6,251 / 7,875 / 1,475 |
| Delayed P2 abort, CAS fencing disabled | 2,326 / 2,770 / 1,468 |
| P5 commit before cut barrier | 235 / 3,117 / 8,386 |
| P5 delta omits a dirty chest update | 19,269 / 18,366 / 14,512 |
| P5 replay scans beyond a torn record | 1 / 0 / 1 |
| P5 stale process appends to the log | 3,154 / 10,298 / 13,742 |

The torn-replay mutation is rare in this workload. Witnesses are setting 0 seed
17,289 and setting 2 seed 495. The normal short mutation test reaches the latter.
Zero failures at setting 1 do not mean that mutation is safe. The split-save
mutation combines current world state with the older resident image; the recovery
mutation instead replaces matching residents from their older store records.
They are separate injected code paths even where their counts coincide.

## Commands, outcomes and limits

Run from `$REPO`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
/usr/bin/time -p just test
cargo test --workspace --all-features
just store-torture
python3 -B scripts/e5-store.py --only latency \
  --latency-driver target/release/goliath-store-load \
  --output docs/experiments/data/E9/rust-driver
cargo build --release -p goliath-sim
target/release/goliath-sim sweep 66667 docs/experiments/data/E9/sweep.csv
target/release/goliath-sim sweep 66667 docs/experiments/data/E9/instant-abort.csv 0
cmp <(sed '/^P5,/d' docs/experiments/data/E9/instant-abort.csv) \
  docs/experiments/data/E1-results.csv
target/release/goliath-sim mutations 20000 docs/experiments/data/E9/mutations.csv
target/release/goliath-sim replay P3 33 0
"$HOME/.agents/skills/rust-project-architecture/scripts/audit-rust-layout.sh" .
git diff --check
scripts/clean-builds --all
```

Formatting, all-feature Clippy, normal and all-feature tests, store torture,
mutation checks, replay, exact E1 comparison and diff checks passed. The short
suite has 34 tests. The final timed run was 4.89 seconds, including Cargo and
doctests, below 30 seconds. Final verification and cleanup are recorded below.

The default fault sweep exits 1 because P3 fails, after writing its full CSV.
The instant-abort sweep exits 0. The sweep failure was not hidden or converted to
success. The Python store latency result also misses its pass line; the torture
harness reports percentiles and does not assert a latency threshold.

Initial Clippy rejected a test module before an implementation block. It was
moved, without a lint suppression. Applying the full P3 abort delay broke two
old tests that assumed P3 safe. Those checks now retain instant-abort P3 coverage
and explicitly require its delayed-abort counterexample. P2/P4/P5 safety checks
remain on normal delayed-abort settings. The final architecture audit found no
missing ownership docs, kind-named modules, wildcard imports, lint suppressions,
undocumented unsafe blocks or library-owned threads. The existing Darwin FFI
boundary is unchanged.

These results do not prove unbounded distributed safety, actual tile-log byte
encoding, disk loss survival, truthful hardware barriers, store replication,
large-state performance or real Horizon integration. P5 assumes immutable dirty
references, complete deltas, correctly framed records, generation-aware recovery,
and process fencing. No real Horizon cut, Minecraft client gap, compaction IO
contention or full-server rollback was tested here. P5's assumed worker delay and
atomic compaction install are not measurements of the new Horizon implementation.
Normal-action progress may roll back in P5. The model's finite item identities,
players, fault windows and scenarios retain E1's other limits.

## Disk and retained evidence

All run folders created by the store harness and short tests lived under
`.local/runs/` and were removed. Other agents' active runs were left alone.
Raw E9 evidence is about 820 KiB, including failed and intermediate timings.
No result contains a home path. The largest sampled E9 run allocation was
8,392,704 bytes across the intermediate and final experiments; the final full
run reached 8,257,536 bytes. Sampling every 10 ms can miss shorter peaks.

The largest `du` sample of the workspace target was 270,432 KiB, about 264.1 MiB.
The harness's file-block sum reached 356,605,952 bytes, but may count Cargo hard
links twice. It is not a measurement of unique physical allocation.
A shared repository sample reached 5,672,908 KiB, with `.local` at 5,383,036 KiB.
These shared sizes include concurrent E6/E7/E8 work and are not attributed to E9.
No extra target directory or reference checkout was created for this experiment.

After the final formatting, all-feature Clippy, 34 normal tests in 4.89 seconds,
34 all-feature tests and diff checks passed, `scripts/clean-builds --all` removed
the idle workspace target and reported 265 MiB freed. No Cargo or experiment
process was running for E9. The script skipped the in-use Horizon, Horizon-tiles
and Meridian targets, reported at 1,385, 1,262 and 476 MiB. Those belong to other
agents. The script reported the shared project at 5,353 MiB immediately afterwards.
Deleted build output is reproducible. No source or retained binary was deleted.

The sampled shared repository peak was about 5.41 GiB, not a continuously measured
whole-machine peak. Final command outcomes and binary/result hashes are also in
`data/E9/verification.json`. No E9 run folder or compiled binary was retained.

## Independent verification

The reviewer reran the simulator results on 2026-11-07. The default sweep exited 1 as expected because P3 fails, and its CSV matched `data/E9/sweep.csv` byte for byte, SHA-256 `0c92e6ed...`. The instant-abort sweep without P5 matched E1's CSV, `e70e7e27...`. The 540,000-scenario mutation run matched `data/E9/mutations.csv`, `90a879fd...`, and `replay P3 33 0` gave hash `9762ffd904c8e02f`. Formatting, all-feature Clippy and 34 tests passed.

The store latency line is not accepted yet. It needs a rerun on a quiet machine with one agreed driver, and the M2 item stays open.
