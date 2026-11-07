# E5: durable ownership store

## Result

Implemented `crates/goliath-store`, a single local service for tile process tokens
and player records. The retained run passed 1,000 SIGKILL cuts under eight concurrent
TCP clients, with zero lost acknowledged commits, backward generations or owner
violations. Every byte of a 154-byte final record was tested for truncation and
single-byte corruption. The fence-disabled control failed the same stale-request
safety assertion that the normal store passed.

The honest commit P95 on this Mac is 3.954 ms at one client, 34.080 ms at eight
clients and 222.896 ms at 64 clients, using `F_FULLFSYNC`. At 64 clients this store
alone misses the 150 ms crossing budget. Plain `fsync` looks much faster but is not
the durability number to use.

Implementation and measurements are complete, pending review. No milestone
boxes were ticked. Nothing was committed or sent to GitHub.

## What was built

The dependency direction is `goliath-store -> goliath-model`. The simulator also
uses `goliath-model`; it does not depend on the real store.

`Record<B>` now accepts either E1's fixed inventory array or a versioned opaque
`Blob`. `OwnershipCas::commit` owns generation comparison, owner changes,
overflow rejection, both fence checks and equal-payload duplicate detection.
`accepts_fence` owns token equality. The real store and simulator call these same
functions. The simulator's existing deliberate fence mutation remains available.
No simulator scheduling or workload code changed.

| Owner | Responsibility |
| --- | --- |
| `goliath-model::OwnershipCas` | Shared generation and fence rules |
| `state.rs` | Directory entries, opaque records and retry receipts |
| `journal.rs` | Checksums, prefix recovery and snapshot installation |
| `durable.rs` | Safe file barrier API and the small Darwin FFI boundary |
| `codec.rs` | Versioned binary encoding and whole-frame IO deadlines |
| `engine.rs` | Validate, persist, publish and fail closed after IO failure |
| `client.rs` | Blocking calls and unchanged-request reconnects |
| `main.rs` | Flags, loopback listener, mutex and connection threads |

There are no external Cargo dependencies. The new crate is a real OS persistence
and service boundary, not a tree-cleanup split. All crates remain application
internal and inherit workspace lints. Library code spawns no threads. The binary
admits at most 128 active connections and owns all connection threads.

### Operations

| Operation | Contract |
| --- | --- |
| Claim tile | Allocate a persistent higher epoch and globally increasing process token. Replace the previous holder atomically. |
| Read directory | Return the current entry per tile, including active state and heartbeat counter. |
| Read player | Return one record or absence, with generation, owner tile, blob version and bytes. |
| Create player | Require a current owner fence and absence. Create generation zero. |
| Commit handover | Check current source and target fences, then compare source owner and generation. Install the next generation and blob atomically. |
| Release | Deactivate a tile's exact current token. Keep its epoch history and player records. |
| Heartbeat | Require an active current token and retain the maximum supplied counter. |
| Compact | Install a snapshot and reset the journal without changing logical state. |

Heartbeats are informational, not expiring leases. A controller must explicitly
claim a replacement. An old process cannot continue committing after replacement,
but stopping that process's local game simulation is outside this service.

Every mutation carries a nonzero, globally unique caller-owned 128-bit request
ID. The caller must reuse both ID and complete payload after an uncertain result.
Successful mutation receipts survive restart and compaction. Reusing an ID with a
different payload is rejected. An exact older retry can return its original result
even after later generations commit, without changing the latest record.

Both fences are checked before looking up a commit receipt. A replaced process
gets `StaleFence` even when retrying a previously successful command. The retry
receipt does not grant authority. Released tokens also cannot commit or heartbeat.

### Disk ordering

One mutex serializes reads and writes, and a separate stable `writer.lock` inode
prevents two store processes from opening the same directory. Every journal record
has an eight-byte format marker, sequence, length and complement, CRC-32 and
complement, then the encoded command. CRC covers sequence, length and payload.

A mutation is validated first. The command is appended and its selected durability
barrier must succeed before the state or receipt becomes visible. Only then can
the binary reply. An IO error marks the engine failed and terminates the service;
it does not continue serving potentially divergent memory.

Recovery reads a checksummed snapshot and replays consecutive journal sequences.
An incomplete or checksum-invalid tail is truncated to the valid prefix and synced
before accepting more operations. A bad record with a valid successor is treated
as interior corruption and refuses startup. A corrupt snapshot also refuses startup.

Compaction writes `snapshot.next`, syncs it, renames it to `snapshot`, syncs the
directory, then truncates and syncs the journal. A crash between snapshot install
and truncation leaves old records that recovery skips by snapshot sequence.
Incomplete future `snapshot.next` files are ignored. Directory creation and new
journal entries also receive directory barriers. The parent directory must exist.

Retry receipts are retained indefinitely. Compaction removes journal history but
does not garbage-collect receipts or their payloads. Long-lived production use
needs a specified retry-retention horizon and receipt collection before storage
can be bounded. The current experiment makes no bounded-growth claim.

### Protocol and client

TCP is restricted to loopback. Each connection carries one request and one response.
Frames use a four-byte little-endian payload length, protocol version 1 and explicit
operation tags. Numeric fields are little endian. Reads reject unknown versions,
unknown tags, trailing bytes and oversized lengths. Frames are capped at 1 MiB;
opaque blobs at 256 KiB. The directory response is not paginated and must fit the
frame limit. This is a trusted-local protocol, not an authenticated network API.

`Client::new(address, timeout, attempts)` requires explicit nonzero limits.
Connect, whole-frame write and whole-frame read each have their own timeout.
Partial reads do not reset the frame deadline. The maximum nominal wait is three
timeouts per attempt, plus local scheduling. Transient IO failures reconnect with
the identical command; application faults and malformed responses are not retried.
Exhausted attempts mean the result is unknown, not that the mutation was aborted.
Callers should retain the command, retry it or read current state, and never invent
a new request ID for an uncertain commit.

Example launch, with the run parent already present:

```sh
cargo run --release -p goliath-store -- \
  --dir .local/runs/manual-store --listen 127.0.0.1:0 \
  --sync full --compact-every 1024
```

The service prints its actual `LISTEN` address. Port zero avoids collisions with
parallel game servers. `--compact-every 0` disables automatic compaction, not the
explicit compact operation. `--sync full` is the default. `--sync fsync` exists for
the weaker-barrier comparison. Only the `fault-injection` build accepts
`--fault-disable-fence`; normal builds reject that flag.

## Process evidence

Measured on 2026-11-07, Apple M5 Pro, macOS 26.5.2, arm64, Rust 1.98.1. The service
used the release profile. The Python standard-library harness used real subprocesses
and sockets, not an in-memory store. Its observer stayed alive while the store alone
received SIGKILL. The retained full run took 89.264 seconds, excluding compilation.

| Seed | SIGKILL cuts | Concurrent clients | Load commits acknowledged | Transport-unknown calls | Missing requests completed by retry | Snapshot versions observed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 5001 | 500 | 8 | 2,082 | 4,000 | 3,502 | 95 |
| 5002 | 500 | 8 | 2,137 | 3,988 | 3,486 | 95 |

All 1,000 cuts had zero lost acknowledgements, backward generations and owner
violations. The observer keeps per-player acknowledged and observed generations.
Before retrying anything after each restart it checks that recovered generation is
at least both values. It checks deterministic blob contents, expected owner at each
generation, unique tile/process directory entries and persistent tile tokens.
It then retries each last command with its exact original ID and payload.

The missing-request column counts requests not present at recovery that a retry
then committed. It does not count lost acknowledged writes. The 4,219 load
acknowledgements exclude successful replay/retry calls between kill windows.

Kill delays are seeded random values from 0.1 to 25 ms after eight clients start
issuing commits. Actual IO overlap is OS-dependent. Automatic compaction runs every
64 mutations. This exercises process cuts during live work and compaction; it is
not exhaustive coverage of every instruction or filesystem crash state.

### Stale requests and the control

The client sends a frame length and partial request body before replacing a tile.
It holds the remaining body, claims a new token, delivers a later fresh heartbeat,
then finishes the old CAS request. Cases alternate replacement of source and target.
The same stale command is delivered again. Old-token release, heartbeat and create
requests are also rejected.

| Build mode | Delayed CAS requests | Rejected | Stale accepted | Same safety assertion |
| --- | ---: | ---: | ---: | --- |
| Fencing on | 32 | 32 | 0 | Pass |
| Fence deliberately disabled | 32 | 0 | 32 | Fail as required |

The explicit control rerun records `stale CAS accepted: 32/32` as the caught
assertion failure in `fence-results.json`. The harness exits successfully only
because this negative control is required to fail. An undetected mutation fails
the harness. The selected control is the fence mutation, not a no-fsync mutation.
SIGKILL does not discard the kernel or drive cache, so a no-fsync process-kill run
alone would not establish power-loss durability.

Short real-process tests also race eight distinct CAS payloads against one player,
with exactly one winner, and eight claims for one tile, leaving exactly one token
authorized to heartbeat. A TCP fault proxy discards a real committed response and
proves the Rust blocking client's reconnect returns the original result once.

### Torn tails and compaction cuts

The fixture starts from a valid snapshot and one final 154-byte commit record.
For every offset 0 through 153 the harness truncates there, then separately flips
one bit in that byte. Each case starts a real store, checks the old generation,
checks that the bad tail was removed, retries the command, kills the process and
starts it again to check the new generation and idempotent response.

| Cases | Count | Outcome |
| --- | ---: | --- |
| Tail truncations | 154 | All rejected and recovered to the valid prefix |
| Single-byte tail corruptions | 154 | All rejected and recovered to the valid prefix |
| Restarts including complete-record control | 617 | All passed |
| Complete valid final record | 1 | Retained, not dropped |

These mutations treat the final damaged record as an unacknowledged interrupted
write. Deliberately corrupting an already acknowledged record on stable media is
not covered by the no-lost-acknowledgement result. CRC-32 is error detection, not
authentication or protection against every possible corruption.

The short suite additionally reconstructs snapshot-installed/journal-not-truncated
state and an incomplete next snapshot, then restarts, appends and restarts again.
Separate real process starts reject interior journal corruption and bad snapshots.
The journal lock is tested against a second writer.

## Commit latency

The measured interval starts before TCP connect and ends after decoding the commit
response. It includes socket setup, server queueing, the real disk barrier and
automatic compaction every 256 mutations. Each client has one outstanding request
and a separate player. Blobs are 256 bytes. Each client warms one commit before the
timed set. Percentiles use nearest rank over all timed samples, without trimming.
Python thread scheduling and the ten-millisecond disk sampler are included.

| Barrier | Clients | Samples | P50 ms | P95 ms | P99 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Plain `fsync` | 1 | 1,024 | 0.160 | 0.210 | 0.257 |
| Plain `fsync` | 8 | 1,024 | 1.942 | 3.102 | 3.692 |
| Plain `fsync` | 64 | 2,048 | 12.588 | 22.794 | 28.788 |
| `F_FULLFSYNC` | 1 | 1,024 | 2.999 | 3.954 | 6.067 |
| `F_FULLFSYNC` | 8 | 1,024 | 24.034 | 34.080 | 39.854 |
| `F_FULLFSYNC` | 64 | 2,048 | 196.031 | 222.896 | 228.978 |

The Mac adapter calls the C `fsync` function directly for the weak comparison and
`fcntl(fd, F_FULLFSYNC)` directly for the durable mode. It does not infer their
semantics from Rust's platform adapter. Full mode has no silent fallback. Directory
updates also use explicit barriers. Apple's [fsync manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsync.2.html)
explains that `fsync` does not itself flush the drive cache. Its
[fcntl manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html)
documents the additional flush request made by `F_FULLFSYNC`.

Use the full-barrier rows for D6's budget. One client spends about 3.954 ms of a
150 ms component budget at P95, eight spend 34.080 ms, and 64 exceed it by 72.896 ms.
The store currently serializes one flush per mutation. Group commit is a future
optimization, not part of this result. These component percentiles do not prove
an end-to-end P95. The coherent tile cut, tick alignment, proxy and client work
must be measured together, especially while other agents' real game servers run.

## Commands and verification

Commands run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings
/usr/bin/time -p just test
cargo run --release -p goliath-sim -- sweep 66667 .local/runs/E5-E1-results.csv
shasum -a 256 .local/runs/E5-E1-results.csv
just store-torture
python3 -B scripts/e5-store.py --only fence
$HOME/.agents/skills/rust-project-architecture/scripts/audit-rust-layout.sh .
git diff --check
```

The normal suite passed all 26 tests in 1.92 seconds wall time, including Cargo
and doctests. It contains no long kill loop. `just store-torture` builds the explicit
test-feature binary and runs the full independent process experiment. Targeted
`--only kills --kills 10` and `--only fence` smoke checks also passed before it.

E1's 1,000,005-scenario sweep reproduced SHA-256
`e70e7e27b0bdd11566b590166a137a59df3b71d3f250172f7cd0de68150c66b7`
exactly. Its temporary reproduction CSV was removed, leaving the original E1 data
unchanged. The stricter equal-payload retry rule did not change any sweep row.

The architecture audit reports no missing module ownership docs, undocumented
unsafe blocks, wildcard imports, kind-named modules or lint suppressions. The one
unsafe block is the documented Darwin syscall boundary. Initial gate attempts
failed on a collapsible match and test-helper unwrap/dead-field lints. Those were
fixed without suppressions. No safety test failed except the deliberate control.

Raw evidence is in [data/E5/](data/E5/). `all-results.json` holds the retained full
run and measured binary hash. `latency.csv` holds all 8,192 timed samples, `tail.csv`
all 308 mutation cases, and `fence-results.json` the explicit shared-checker rerun.
The raw files total about 208 KB. No worlds or service logs are retained.

## Limits

- This is one store process on one local disk, a single point of failure. There is
  no replication, consensus, failover, backup restoration or protection against
  losing the disk. Restart recovery is not high availability.
- SIGKILL is not a Mac power cut or controller-cache loss. The code issues the
  documented full barrier, but actual power-loss survival and truthful hardware
  completion were not physically tested.
- Horizon's coherent tile checkpoint and P2 ordering are not implemented here.
  There are no Minecraft inventory, item conservation or whole-world assertions
  in E5. The blob's producer must make the correct checkpoint first.
- No Horizon, Meridian or proxy integration was attempted. No Horizon or Meridian
  source files were changed by this task. Linux and Windows runtime behavior was not
  tested. No Folia, Paper, GPL source or prohibited research report was read.
- Timing covers small blobs, a small player population, a short run and closed-loop
  concurrency. It is not an open-loop throughput or long-soak result. Other agents
  were running real game experiments on this shared Mac, so it is not an isolated
  hardware benchmark.
- Receipts grow without bound. Recovery and snapshot encoding load complete data
  into memory. Large-state latency, disk-full handling, malicious local clients,
  authentication, receipt collection and broad fault injection for OS errors remain
  unverified. Journal and snapshot format migration is not implemented.

## Disk and cleanup

The full experiment's sampled run-directory peak was 4,341,760 allocated bytes,
about 4.14 MiB. Sampling ran every 10 ms and can miss shorter peaks. The shared
workspace target and repository sizes are recorded at final cleanup below; other
agents were simultaneously growing `.local`, so their disk use is not attributed
to E5. Every E5 run directory was deleted after its test. Other agents' active run
directories were left alone. No E5 binary is retained outside disposable `target/`.

After the final gates, with no Cargo command running for this task:

```sh
scripts/clean-builds --all
```

The script reported the workspace target at 132 MB and removed it. It also removed
the idle 560 MB Meridian target, skipped the in-use 1,834 MB Horizon target, and
reported 692 MB freed with the shared project at 2,589 MB afterwards. These were
disposable build artifacts, not source or retained binaries. They can be rebuilt.
The largest observed shared repository size before cleanup was 3,319,012 KiB,
about 3.17 GiB. This was sampled during parallel E3/E4 work, not a continuously
measured whole-repository peak. E5's own run-directory peak is the separately
sampled 4.14 MiB above. The final normal and all-feature Clippy checks, formatting
check and `git diff --check` all passed before cleanup.

## Independent verification

The reviewer reran `just store-torture` on 2026-11-07. It exited 0 with 1,000 kills, zero lost acknowledgements, backward generations and owner violations. `F_FULLFSYNC` P95 was 4.2 ms at 1 client, 34.4 ms at 8 and 257.0 ms at 64, against 4.0, 34.1 and 222.9 in the first run. Raw data is in `data/E5/repro-*`. E9 replaces the per-mutation flush with group commit.
