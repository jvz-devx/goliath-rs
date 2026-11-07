# E15. Store commit latency

Implementation evidence. Review and commits are separate. No source
from Folia, Paper, GPL projects or the excluded research report was read.

## Result

The store line passed three separate, untraced Rust-driver runs. At 64 clients,
commit P95 was 15.971, 15.940 and 16.100 ms with `F_FULLFSYNC`. Each run contains
8,192 timed commits. All three quiet acknowledgements remained present and the
one-minute load stayed below 4 throughout the accepted runs.

The persistent-transport 1,000-kill campaign, controls, short tests and a real
14-crossing cluster proof passed. Review is separate. No milestone
box is ticked by this implementation report.

## Breakdown before changes

The instrumented E9 service reproduced the miss at 64 clients with the Rust
driver. Each client had one outstanding commit, a separate player and a 256-byte
blob. Every call opened a new TCP connection. The timed interval includes connect,
encoding, send, response decoding and response checking. Percentiles use nearest
rank over all samples without trimming. Automatic compaction ran every 256
mutations unless a diagnostic explicitly disabled it.

| Diagnostic | Samples | P50 ms | P95 ms | P99 ms | One-minute load before |
| --- | ---: | ---: | ---: | ---: | ---: |
| Instrumented original service | 4,096 | 12.754 | 30.085 | 36.873 | 2.63 |
| Same service, compaction disabled | 4,096 | 8.039 | 9.920 | 11.111 | 2.70 |

The first trace recorded 16 compactions taking 13.847 to 25.041 ms. The worker
could not process the next batch during that work. Maximum request queue wait
was 30.562 ms. Journal barrier P50/P95 was 3.223/4.136 ms across all batches.
Connection admission and decoding P99 was 0.218 ms. The nominal 500 microsecond
collection window took about 0.75 ms once timer wakeup overhead was included.
These phase percentiles describe batches, not an additive decomposition of the
end-to-end percentile.

The no-compaction diagnostic isolates the largest cause. Compaction is enabled
in acceptance runs. Its disabling is not an optimization or an acceptance result.

## Accepted latency runs

Measured on the Apple M5 Pro, macOS 26.5.2 arm64, Rust 1.98.1, across
2026-11-07 and 2026-11-08. Acceptance uses the Rust driver only. Each row starts
a separate store. Each client has one outstanding commit and its own warmed,
persistent handle. Blobs are 256 bytes. Compaction runs every 256 mutations.
No timed sample is trimmed. Percentiles are nearest rank over the complete row.
No final run was disturbed or trimmed.

| Clients | Run | Commits | P50 ms | P95 ms | P99 ms | One-minute load before/after/peak |
| ---: | --- | ---: | ---: | ---: | ---: | --- |
| 1 | final-clients-1 | 1,024 | 3.920 | 4.120 | 7.937 | 3.475 / 3.677 / 3.677 |
| 8 | final-clients-8 | 2,048 | 7.066 | 12.005 | 14.913 | 3.677 / 3.703 / 3.703 |
| 64 | final-1 | 8,192 | 11.030 | 15.971 | 16.903 | 3.690 / 3.475 / 3.690 |
| 64 | final-2 | 8,192 | 11.790 | 15.940 | 16.983 | 3.475 / 3.475 / 3.475 |
| 64 | final-3 | 8,192 | 11.887 | 16.100 | 17.880 | 3.475 / 3.475 / 3.475 |
| 256 | final-clients-256 | 16,384 | 11.846 | 15.994 | 18.847 | 3.703 / 3.703 / 3.703 |

The 64-client P95 range is 0.160 ms. The largest final 64-client call was
18.184 ms. This does not promise every future call is below 20 ms. The 256-client
row is one observation, not a repeated scale guarantee.

An earlier persistent candidate measured P95 16.021 ms, but its one-minute load
rose from 3.957 to 4.921. It was marked disturbed and retained under
`persistent-1/`. Its replacement is `persistent-1-retry-1/`. The accepted runs
from that superseded binary are listed in
[data/E15/persistent-quiet-summary.json](data/E15/persistent-quiet-summary.json).
That binary, `d2fe29`, passed 16.176/16.039/16.150 ms before the metadata-error
guard was tightened. Those results and its executable are retained, not deleted.
The exact final binary's accepted runs are listed in
[data/E15/final-quiet-summary.json](data/E15/final-quiet-summary.json).
Every row's CSV count, percentile, quiet eligibility and executable hash was
checked independently against that summary.

The final quiet window began at 00:10:41 and was released at 00:14:16.491 local
time on 2026-11-08, after 215.491 seconds. Request and E12/E13/E14 acknowledgements were deleted
immediately after the sixth check. Marker and load monitoring ran every 20 ms.
Top-level command-start load metadata can precede the wait; the table uses each
row's post-gate load. Five- and fifteen-minute averages were still elevated from
the preceding shared work. The agreed gate is the one-minute average.

During the final first run, `netstat -an -p tcp` observed 108 TIME_WAIT sockets
host-wide at 0.529 seconds. All timed Rust connect phases were zero, since the
handles were already connected. The earlier accepted first run observed 217,
and the earlier disturbed candidate observed 88.
These are snapshots, not a continuously measured socket maximum.

## Variance and diagnostic runs

The original 23 to 68 ms measurements cannot be attributed retrospectively without
their phase traces. The new baseline trace does show a mechanism for long tails.
Snapshot encoding and multiple barriers block the only store worker while all
clients queue. Receipt retention makes later full snapshots larger. The batching
timer adds a small fixed delay and timer wakeup variance. Those costs are now
removed or moved off the worker, except state snapshot encoding and one rotation
directory barrier.

An intermediate rotation pilot measured 16.153 ms P95 at load 3.50 before empty
journal preparation was moved off-path. It is retained, but not used as one of
the final three trials.

The first coordinated window still used fresh connections. Its traced row had
P50/P95/P99 27.782/57.082/68.893 ms. This is diagnostic evidence only, not a pass
or fail against the agreed untraced criterion. Per-connection CSV writes changed
admission and batch formation. Admission P95 was 49.191 ms, while response-thread
wakeup P95 was 0.103 ms and response writing P95 was 0.048 ms. Multi-request batches
had median size 9; barrier P95 was 7.277 ms. The trace observer materially changed
the workload and must not be used as a clean latency measurement.

The next fresh, untraced row measured 18.118 ms P95. Its connect P95 had risen to
2.966 ms. A third fresh driver exited with a transport error. Its stderr was not
retained by the old harness, so the exact OS error is unverified. The Mac's observed
ephemeral range was 49,152 through 65,535, or 16,384 ports, and TCP MSL was 15,000 ms.
Fresh-connection churn is consistent with port pressure, but this is not a captured
EADDRNOTAVAIL diagnosis. The harness now preserves driver stderr on future failures.
The first window released its markers in its failure cleanup.

Persistent handles remove per-commit connect cost and socket churn in the real
client, not only in a special benchmark transport. The one-client median is
consistent with a roughly 4 ms barrier cadence. Batching waves and periodic
rotation explain the higher concurrent medians and tails. Background snapshot
barriers still compete for the same SSD.

## Changes

The binary drains commands already queued when the previous barrier completes.
There is no intentional timer delay by default. An idle worker blocks for the
first request, then drains the queue up to the existing 256-command cap.
`--batch-us 500` retains the old timer policy for comparison.

Automatic compaction rotates the durable prefix to `journal.previous`. An empty
`journal.next` receives its file barrier at startup or on the snapshot worker,
before it is needed by rotation. It becomes `journal`, and the
directory receives its barrier before new commands append. Snapshot capture and
rotation remain on the store worker. Snapshot framing, checksum, file write,
barrier, rename, directory barrier and retired-prefix removal run on a separate
binary-owned worker. At most one snapshot worker runs at a time.
Preparing the next empty journal on that same worker also removes its file
barrier from the next rotation's latency path. Rotation still syncs the directory
before any command can use the new journal.

The worker installs a snapshot for the captured sequence only. It never truncates
the new journal. Newer acknowledged commits and retry receipts remain there.
The old prefix is removed only after snapshot installation is durable. Startup
replays the previous prefix followed by the active journal, skips records covered
by the snapshot, and completes an interrupted rotation before serving requests.
It refuses a corrupt rotated prefix rather than treating it as an unacknowledged
active tail. Explicit `Compact` waits for an in-flight snapshot and retains its
complete-before-reply contract.

`Engine::begin_maintenance`, `Compaction::run` and
`Engine::complete_maintenance` expose this caller-driven work without giving the
library threads or a runtime. The existing synchronous engine API remains.
The store process exits on snapshot or rotation IO failure. Batch barrier errors
still return no responses and poison the whole engine, including reads.
Rotated-prefix presence uses `try_exists`, so metadata IO errors propagate instead
of silently pretending that an interrupted rotation is absent.

Wire tags, frame encoding and existing client method signatures are unchanged.
E10 UUID and clock operations are unchanged. `Client::new` now keeps one TCP
connection per handle and serializes that handle's calls. Any IO or decoding
error discards the stream. Eligible retries reconnect with the exact original
request ID and payload. `Client::fresh` retains the original transport, and
`goliath-store-load --fresh` keeps it testable.

The server reads repeated frames and accepts pipelined existing frames, replying
in order. The driver has one outstanding commit per simulated client. It does
not pipeline. Each client owns a separate handle, warmed before the timed start.
The binary caps live connection workers at 512 instead of letting persistent
handles occupy every slot of the old 64-worker pool. Idle handles can wait for
their next frame; the whole-frame five-second deadline starts when data arrives.
The existing frame and queue caps remain. Library code owns no threads.

Source and target fences are still checked before receipts on every CAS, not
once per connection. A receipt does not confer authority on a stale process.

`GOLIATH_STORE_TRACE` enables per-batch phase CSV and a separate connection CSV
for response-thread wakeup and response-write timing. `Client::call_with_timing`
adds successful-attempt connect, write and read timing. The Rust driver's original
CSV columns remain first, with phase columns appended. The logical commit
workload is unchanged; the transport is now persistent.

The Rust architecture skill kept persistence ordering and journal formats in the
library, and scheduling and thread lifetime in the binary. No crate or dependency
was added.

## Short safety tests

Normal and all-feature suites each passed 41 tests. `just test` completed in
4.50 seconds wall time on the final normal-suite rerun, including Cargo and
doctests. Formatting and strict
all-feature Clippy passed without suppressions.

Four added engine tests cover commits and receipts newer than an in-flight
snapshot, five interrupted-rotation states, snapshot write failure, rotation
failure, explicit-compaction exclusion and refusal of a corrupt rotated prefix.
The interrupted states include a missing active journal, a snapshot not installed,
an installed snapshot with the old prefix still present, a torn active tail, and
a cut during startup cleanup after journal truncation.

Existing real-process tests still cover UUID and clock recovery, stale fencing,
conflicting request payloads, competing CAS and claims, exclusive writer locking,
lost-response reconnects, snapshot installation cuts and corrupt storage.
The reconnect proxy test now first warms the reused socket, then drops a real
commit reply. The retry still returns the original committed generation. New
tests prove one connection is reused, fresh mode opens a connection per call,
and two existing wire frames can be pipelined without reordering replies.

## Process safety reruns

The final safety campaign used the release `fault-injection` executable with
SHA-256 `e1ec3a4b0c99e20aeceb4cf69f71bd89611ace0906ca6d05d76c8a4e0df3ce6e`.
The fault flags were off except in their named controls. It completed in
89.324 seconds. Timing was deliberately separated from safety, because other
agents kept the one-minute load above 20 during this campaign.

| Seed | Kills | Clients | Load acknowledgements | Unknown calls | Missing requests completed by retry | Snapshot versions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 5001 | 500 | 8 | 5,790 | 4,000 | 2,124 | 147 |
| 5002 | 500 | 8 | 5,609 | 3,991 | 2,202 | 151 |

Both seeds had zero lost acknowledgements, backward generations and owner
violations. The observer checks recovered state before retrying uncertain
commands. Snapshots remained enabled every 64 mutations during the kill test.
Load workers reuse a socket within each kill window. The independent observer
and recovery retries use fresh connections.

| Check | Outcome |
| --- | --- |
| Delayed stale CAS after source or target replacement | 32/32 rejected |
| Same checker, deliberately disabled fence | 32/32 accepted, failed as required |
| Final-record truncation | 154/154 passed |
| Final-record single-byte corruption | 154/154 passed |
| Tail fixture process starts | 617/617 passed |
| Barrier before acknowledgement, volatile-cache loss | Generation 1 recovered as 1 |
| Early-acknowledgement control, same cache loss | Generation 1 recovered as 0, failed as required |

The separate `--only barrier` rerun repeated both barrier outcomes with the same
executable. Its failure text is `lost acknowledgement after unflushed cache loss`.
This is a deliberately dropped volatile file cache, not an actual power cut.

An earlier complete 1,000-kill safety loop also passed, then was interrupted while
waiting for quiet-host latency. It is not counted as a completed torture command.
The final command used `GOLIATH_STORE_SKIP_LATENCY=1` and exited successfully;
quiet Rust-driver timing is a separate step. E9 evidence was not overwritten.
Raw final safety results are in [data/E15/final-safety/](data/E15/final-safety/),
and the separate control is in [data/E15/final-early-ack/](data/E15/final-early-ack/).
Earlier completed campaigns are retained separately under `safety/`,
`safety-final/`, `persistent-safety/`, `early-ack/`, `early-ack-final/` and
`persistent-early-ack/`.

## E10 integration

The cluster script's owner added `--store-binary PATH` and hashes the selected
executable in its result. The requested proof ran after the quiet window:

```sh
python3 -B scripts/goliath-cluster proof --crossings 14 --name e15-check-final \
  --store-binary .local/bin/goliath-store-e15-final-d4d84912bcd1/goliath-store
```

It completed all 14 crossings with 14 visible CAS operations, generations 1
through 14, matching owner and inventory after every crossing, zero disconnects
and zero corrections. All four tiles restarted, all four world markers remained,
and recovered inventory matched. The independent E10 analyser passed on the
result and verified the selected store hash.

No shared E10 binary was replaced by E15. Current Horizon and Meridian hashes
are recorded in the proof. This is a compatibility and recovery check under
resumed shared load, not a new 1,000-crossing or physics-client performance claim.
Raw proof and analysis are [data/E15/cluster-proof.json](data/E15/cluster-proof.json)
and [data/E15/cluster-analysis.json](data/E15/cluster-analysis.json).
The earlier requested `e15-check` proof also passed on `d2fe29`; its copied
evidence is retained as `cluster-proof-superseded.json` and
`cluster-analysis-superseded.json`.

## Commands and outcomes

Completed commands from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
/usr/bin/time -p just test
cargo test --workspace --all-features
cargo build --release -p goliath-store --features fault-injection
GOLIATH_STORE_SKIP_LATENCY=1 \
  GOLIATH_STORE_OUTPUT=docs/experiments/data/E15/final-safety just store-torture
python3 -B scripts/e5-store.py --only barrier \
  --output docs/experiments/data/E15/final-early-ack
git diff --check
```

Listed verification commands passed. The architecture audit found no missing module
ownership docs, new unsafe blocks, wildcard imports, kind-named modules or lint
suppressions. Traced rows are diagnostics, not acceptance results. The first
window's third fresh driver failed, as recorded above. No final verification
failed. No commit, push, release or GitHub action was made.

The exact store executable is retained at
`.local/bin/goliath-store-e15-final-d4d84912bcd1/goliath-store`, with the full hash
above. The retained `goliath-store-load` SHA-256 is
`0d5a533da7df2e40089595f96636ed1d3e9b0bef9685fe411c87f9b5d5774214`.
After coordinating a fresh request and all three acknowledgements, repeat each
64-client row into a new output directory with:

```sh
python3 -B scripts/e5-store.py --only latency --latency-modes full \
  --binary .local/bin/goliath-store-e15-final-d4d84912bcd1/goliath-store \
  --latency-driver .local/bin/goliath-store-e15-final-d4d84912bcd1/goliath-store-load \
  --latency-clients 64 --latency-samples 128 --quiet-window \
  --output docs/experiments/data/E15/reproduction-1
```

The 1, 8 and 256 rows use client/sample pairs 1/1024, 8/256 and 256/64. The quiet
gate times out missing acknowledgements after 30 minutes and any total wait after
60 minutes. The window owner must release request and acknowledgement files on
completion or failure. The recorded runs did so in an unconditional cleanup.

## Remaining limits

The snapshot still retains every receipt and payload, as in E5 and E9. Storage
and snapshot memory are not bounded for an indefinitely running service. Rotation
still adds file and directory barriers between batches, and background barriers
share the same device with commit barriers. A low load average is not proof of an
idle SSD or an isolated host. Process kills and the volatile-file-cache control
do not establish this SSD's real power-cut behavior.
State snapshot encoding remains serial and grows with retained receipts. These
finite-state runs do not bound latency after indefinite receipt growth.

This store change does not repair or retest E13's separate tile-side append and
ordinary-login fencing defects.

## Cleanup and handoff

E15 run directories, including both cluster proofs, were removed after use.
Quiet requests and acknowledgements were released in unconditional cleanup.
Raw E15 evidence totals about 4.1 MiB. Earlier measurements and executables remain
available as superseded evidence. The final executable and driver remain under
`.local/bin/goliath-store-e15-final-d4d84912bcd1/` for the reviewer's reproduction.

The final build-output cleanup command is plain `scripts/clean-builds`, without
`--all`. No source, retained binary or other agent's live run is a cleanup target.

## Verification status

The reviewer checked the gates on 2026-11-08: formatting, all-feature Clippy and 41 tests passed, and the retained final binary hashes to `e1ec3a4b...`. The 64-client timing isn't reproduced yet; the reviewer will rerun it in the M4 quiet window on this binary before the M2 store item is ticked.
