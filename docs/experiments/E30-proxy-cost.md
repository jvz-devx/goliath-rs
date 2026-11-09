# E30. Proxy cost: where it goes and what to cut next

Status: accounting done from source and retained data. One patch is written and
gated, `patches/meridian-13-manager-park.patch`. It stops the per-player terrain
manager from polling at 50 Hz when it has nothing to do. Nothing is measured yet:
no release binary was built and no cell ran. The reviewer runs the cells in the
[measurement plan](#measurement-plan). No frontier or milestone claim is made.

Line references are to the reactor-1 source: Meridian
`902dcf195e0a8bbd3f39a5f560684af00e64bf5d` plus `e2-meridian-client` and
`meridian-01` to `meridian-06`. Paths are relative to `.local/src/meridian`.

## The proxy's threads

Measured, from the two r2 frontier cells at 425 players on Linux
([r0](data/E28/frontier/e28-frontier-r2-425-r0.json.gz),
[r1](data/E28/frontier/e28-frontier-r2-425-r1.json.gz), key
`cells.425.proxy_thread_cpu`) and the Mac thread-cost readback at 650
([e25-thread-cost-r1](data/E25/e25-thread-cost-r1.json)):

| Thread role | Count | Linux 425 cores, r0 / r1 | Mac 650 cores |
| --- | ---: | ---: | ---: |
| `proxy-io-N` IO drivers | 4 | 0.619 / 0.586 | 0.842 |
| `terrain-manager` | one per player | 0.090 / 0.089 | 0.234 |
| `player <address>` supervisors | one per player | 0.0001 / 0.0001 | 0.0001 |
| `proxy-control-N` command workers | 4 | 0 / 0 | idle |
| Everything else matched | 13 to 17 | below 0.001 | residual 0.042 |
| Whole process | 877 / 879 threads | 0.721 / 0.702 | 1.119, 1,330 threads |

The IO drivers split 0.328 user and 0.292 system on Linux r0. The managers split
0.040 user and 0.053 system there, and 0.037 user and 0.197 system on the Mac.

Why each thread exists:

- Upstream Meridian reads every socket with a blocking thread. A session has a
  supervisor, one thread per backend lane (`session-lane-N`,
  `crates/meridian-handoff/src/session.rs:998`) and one for the client
  (`session-to-backend`, `session.rs:443`). That is the three threads per player
  the earlier docs mention. Each terrain reader added one more
  (`crates/meridian-handoff/src/terrain.rs:374`). Patch 05 at 975 players had
  4.46 threads per player (E18).
- Reactor-1 moved all socket reads and writes to four fixed drivers
  (`crates/meridian-proxy/src/io_runtime.rs:25-65`). Lanes, the client and terrain
  readers register with a driver instead of starting a thread
  (`session/reactive.rs:20-76`, `terrain.rs:359-371`).
- Two threads per player are left. The supervisor
  (`crates/meridian-proxy/src/serve.rs:195`) blocks on the session's event
  channel for the whole session and costs nothing measurable. The terrain manager
  (`crates/meridian-proxy/src/terrain.rs:52`) is the one that costs CPU.
- Wall workers (`walls.rs:68`, `wall_approach.rs:122`) and switch threads
  (`switching.rs:316`) live only around a crossing.

Linux measures 2.02 threads per added player, the Mac 2.03 to 2.04. That matches
one supervisor and one manager.

## What a terrain manager does

`crates/meridian-proxy/src/terrain.rs:65-159` is a loop with a fixed 20 ms sleep
(`terrain.rs:158`). Every pass takes the session's `down` and `up` locks about
five times: position, active backend, radius, reset flag and chunk centre
(`terrain.rs:66-79`). Then it decides for each tile whether the player needs a
terrain reader there (`terrain.rs:80-96`). A player needs a reader only within
radius plus two chunks of another tile. Only a pass that has or wants a reader
talks to a tile: `player-observe` on first need, on a chunk change and every five
seconds (`terrain.rs:105-110`, `141-156`).

In the D9 layout 90% of the players patrol a square at least 300 blocks from any
wall. Their managers never want a reader. Each one still wakes 50 times a second,
takes the locks, finds nothing and sleeps again.

Measured on Linux at 425 (r0): the 425 managers are nearly uniform, 190 to 269
microcores each with a median of 207. The cheapest 383, the size of the interior
group, sum to 0.079 core. The other 42 sum to 0.011. So polling is the cost, and
reader work is a small extra. I can't tell from the record which thread belongs
to which player, so "cheapest 383 are the interior players" is an inference.

The proxy makes 25,837 context switches a second in that cell. 425 managers at
50 wakes a second is 21,250, or 82% of them. That share is arithmetic from the
sleep interval, not a per-thread counter. Those wakes land on CPUs 0 to 3, the
same four cores the tile main threads need.

On the Mac the same threads cost more per wake. `__semwait_signal`, the sleep
call, is 12.19% of the proxy's Running samples at 650
([e25-proxy-profile-via-d2-r2](data/E25/e25-proxy-profile-via-d2-r2.json)), and
the managers' native counters give 0.234 core.

## What a forwarded packet costs

Read from the source. The counts below are per readable event, and no syscall
profile exists on Linux (the cell records `unix_syscalls: null`), so the split
between calls is inferred.

One driver turn (`crates/meridian-net/src/io_pool.rs:410-529`):

1. `epoll_wait` or `kevent`, once per turn for every ready socket
   (`io_pool.rs:510-527`). A turn that queued local writes skips it
   (`io_pool.rs:505-509`), so forwarding inside one driver never pays a wake
   syscall (`io_pool.rs:76-83`).
2. For each readable socket, `read_inner` reads until the kernel says
   `WouldBlock` (`io_pool.rs:551-569`). That is at least two `recv` calls per
   event: one that returns the data and one that returns nothing.
3. Each decoded packet goes through the session. For a clientbound play packet
   that is the `down` lock, lane-bound id mapping, the entity-id and sky
   rewrites, a re-encode, and the ghost view update
   (`crates/meridian-handoff/src/session.rs:1096-1215`). The coalescing writer
   frames it into one pending buffer (`drain.rs:139-164`).
4. At the end of the read burst the writer hands that one buffer to the output
   queue (`drain.rs:91-137`, `session/reactive.rs:95-109`). The driver sends it
   with one `writev` (`io_pool.rs:571-639`).

So a burst costs about one data `recv`, one empty `recv`, one `writev` and a
share of one selector call. Packets inside a burst cost user time only.

Measured shares:

- Mac, 650 players, reactor-1 (E25 profile): `__recvfrom` 22.3%, `writev` 17.4%,
  `kevent` 4.3%, `mach_absolute_time` 2.7%, allocation and copies 11.5%, ghost
  view and rewrite 6.2%. `recv` costs more than `writev` although a send does
  more work, which fits two reads per write. The samples can't tell an empty
  read from a full one.
- Linux, 425 players: the drivers use 0.33 core in user code and 0.29 in the
  kernel. Nothing finer is recorded.

The frontier runner sets `E18_PATH_TIMING=1` for every cell
(`scripts/e20-frontier`, the line that sets `os.environ['E18_PATH_TIMING']`).
With it on, each packet pushes a timestamp (`drain.rs:157-159`), each flush boxes
a closure that records every stamp into a histogram (`drain.rs:98-110`), and each
burst reads the clock. So part of the measured proxy cost is the measurement
itself. How much is unknown. One ablation cell in the plan below bounds it.

## Earlier candidates and why they are not repeated

- Scheduler-2 (`meridian-07`, two files) moved manager passes onto IO-driver
  timers and four shared workers. It cut about 650 threads and 0.09 to 0.19 core
  at 650 on the Mac. It still ran every pass every 20 ms, and its workers spent
  47% of their samples on the shared receiver mutex (E20 profile). It was dropped
  because its one soak build grew 34% in RSS (E25). E30's patch keeps the
  threads and removes the passes instead. It adds no queue, worker or timer.
- Shared views (`meridian-08`, four files) removed readers, not manager polling.
  The CPU saving did not reproduce (0.030 and 0.216 core) and it had terrain
  defects on the way.
- Mimalloc changed only the allocator and doubled the latency tail.
- writev64 raised the descriptor count per `writev` from 16 to 64 on macOS only.
  The coalescing writer already sends one buffer per burst, so more descriptors
  rarely apply. It was never measured.
- Zone pressure was a macOS allocator call, unrelated to CPU.

E20's profile proposal put a ceiling of about 0.07 core on "manager no-op
suppression". That ceiling was for scheduler-2, where the managers were already
off their own threads. On reactor-1, the selected stack, the managers measure
0.234 core on the Mac and 0.090 on Linux.

## Proposals, ranked

### 1. Park settled terrain managers (implemented)

Mechanism: a manager whose pass had no reader, wanted none and saw no reset can
only repeat that pass until one of its inputs changes. It waits on a condition
variable instead of sleeping 20 ms. The session signals it when the position
leaves its chunk column, on a server placement or its acknowledgement, on a new
active backend, on a terrain reset, radius change or first position, and when
the session ends.

Expected saving: on Linux at 425, up to 0.079 core of the proxy's 0.72 (the
cheapest 383 managers), less the 250 ms fallback wake, which is 8% of today's
wake rate. I expect 0.06 to 0.075 core and about 19,000 fewer context switches a
second. On the Mac at 650, where the managers cost 0.234 core, I expect 0.15 to
0.20. Thread count does not change. All of this is expectation.

What could break:

- A missed signal. A manager that is never told about a move would prepare a
  reader late, and a crossing could stall waiting for terrain. Every writer of a
  manager input is listed in the [implementation](#what-the-patch-changes), and
  the manager still looks again every 250 ms.
- Handover and authority ordering are untouched. The patch changes only when a
  manager pass runs. A manager with a reader, or wanting one, keeps the 20 ms
  loop, so everything near a wall behaves as before.
- The wall trigger does not involve the manager.
- Cached terrain: `SetChunkCacheCenter` is sent by the manager when the chunk
  changes. That is one of the wake conditions, and a woken pass runs no sooner
  than the old poll would have.
- Tile maps whose origin is not a multiple of 16 can change a decision inside a
  column. The patch detects that and keeps the fixed poll for them.

Tests: unit tests in both crates and four session tests, listed below. Measured
by the paired cells in the plan.

### 2. Stop reading after a short read

Mechanism: with edge-triggered readiness, a `recv` that returns fewer bytes than
the buffer offered has emptied the socket. The driver can stop there instead of
calling `recv` again to get `WouldBlock` (`io_pool.rs:561-564`). The protocol
reader offers a fixed 64 KiB (`fill_from` in the pinned `horizon-protocol`
`connection.rs`), so the driver can compare the count against it. This removes
one syscall per readable event.

Expected saving: unmeasured. If the empty read is a third of `recv` time, the
Mac profile gives about 0.08 core at 650. On Linux an empty `recv` is cheap, and
I expect 0.01 to 0.03 core at 425. That is below the 0.10 rule from E20 by
itself, so it needs counters first: reads, empty reads and bytes per turn.

What could break: a close or error that arrives with the data. The driver must
keep reading when the event carries a read-closed or error flag, and when the
turn cap ended the read. Ordering and backpressure do not change. Test with the
existing loopback tests plus a peer that sends and closes in one segment, and a
frame split across two segments.

### 3. Start the manager thread only near a wall

Mechanism: after proposal 1 a settled manager is a parked thread. The session
could start it on the first signal that puts the player near another tile and
let it exit when it settles. Interior players would then have one thread, so
the proxy would run about 1.1 threads per player instead of 2.

Expected saving: no CPU beyond proposal 1. It removes about 0.9 threads per
player and their stacks. Worth doing only if thread count or RSS becomes the
limit.

What could break: the start decision would run on an IO driver, which must not
block, and a start racing a crossing could prepare a reader late. This is a
smaller version of what scheduler-2 tried, so it needs the full M2 crossing
suite, not only unit tests.

### Not proposed

Removing the supervisor thread saves no CPU (0.0001 core). Fewer IO drivers can
be tried with the existing `E18_IO_THREADS`, without a patch, but the drivers
only account for about 4,600 context switches a second on Linux, so there is
little to gain. Batching sends across turns needs a timer, which E18 ruled out
for latency.

## What the patch changes

`patches/meridian-13-manager-park.patch`, SHA-256
`94e89066bf8d20ec8b6eb7b591cd578cdbd4a620b2c979ff9e3e48b0781735c9`, applies after
`meridian-06-io-pool.patch`. Five files, 317 added lines, about 200 of them
tests. No dependency, no new thread, no `unsafe`.

- `crates/meridian-handoff/src/terrain.rs`: a `Wake` (a counter under a mutex,
  and a condition variable), `SessionHandle::terrain_generation` and
  `terrain_wait`, and `left_cell`, which says whether a new position left the old
  one's column. `initial` now returns whether it changed an input.
- `crates/meridian-handoff/src/session.rs`: the session bumps the counter after
  each write of a manager input. Those are the client pose when it leaves its
  column, a server placement, the acknowledgement that clears it, a reconnect
  that clears the pose, `set_backend`, the three places that mark the client
  gone, and `initial` (respawn, login radius, first position).
- `crates/meridian-proxy/src/terrain.rs`: the manager reads the counter before
  any input. After a settled pass it waits on that value for at most 250 ms.
  After a wake it sleeps out the rest of the 20 ms, so no pass runs sooner than
  the poll would have run it. Any other pass sleeps 20 ms as before.
  `E30_TERRAIN_POLL=1` restores the fixed poll in the same binary.

Why the wake is safe: the manager reads the counter first and its inputs after.
A writer changes its input first and bumps the counter after. So a change either
is read by the pass or makes the wait return at once. A position reaches every
manager decision through its chunk column when the tile origin and edge are
multiples of 16, which `with_aligned_tiles_nearness_is_the_same_across_a_column`
checks over about 250,000 positions. A position that is not finite always wakes.

New tests:

- `meridian-handoff`, `terrain::tests`: `a_position_wakes_the_manager_only_when_it_leaves_its_column`,
  `only_the_first_server_position_is_a_changed_input`.
- `meridian-handoff`, `tests::terrain_wake`, each on a real session over pipes:
  `a_move_ends_the_wait_only_when_it_leaves_the_column`,
  `a_change_during_a_wait_ends_it_before_the_limit`,
  `a_server_placement_and_its_acknowledgement_each_end_the_wait`,
  `a_new_backend_and_the_end_of_the_session_each_end_the_wait`.
- `meridian-proxy`, `terrain::tests`: `a_manager_waits_only_with_no_reader_no_wish_and_no_reset`,
  `with_aligned_tiles_nearness_is_the_same_across_a_column`,
  `with_unaligned_tiles_nearness_can_change_inside_a_column`.

No test starts a real manager thread against a cluster. That needs tiles and is
covered only by the cells below.

## Gates

`scripts/boat-gates reactor-2` rebuilds reactor-1 from the pin and its recorded
patch hashes, checks every compiled source hash, applies patch 13 and checks the
five changed files against
[reactor-2-inputs.json](data/E30/reactor-2-inputs.json). It then runs reactor-1's
gate set and three more: `rustfmt` on the changed files, Clippy with
`-D warnings` on `meridian-handoff` and `meridian-proxy` alone, and the new tests
by name. All gates use the `fast` profile on a boat sandbox. No release build.

Run 2 passed. Record: [reactor-2-gates.json](data/E30/reactor-2-gates.json),
rustc 1.99.1, runner SHA-256 beginning `7407fffa`.

| Gate | Result | Notes |
| --- | --- | --- |
| `proxy-fmt` | accepted, exit 1 | the known formatting diff in `meridian-testkit/src/bin/e2-client.rs` only, as for reactor-1 |
| `proxy-scoped-fmt` | passed | |
| `proxy-clippy` | accepted, exit 101 | the known `fetch_update` deprecation in `meridian-net/src/io_pool.rs` only, as for reactor-1 |
| `proxy-tests` | passed | 1,107 tests, reactor-1's 1,098 plus the nine new ones |
| `reactor-2-fmt` | passed | the five changed files |
| `reactor-2-clippy-changed` | passed | no messages; both changed crates compiled, lib, binary and test targets |
| `reactor-2-park-tests` | passed | 13 tests by name: the nine new ones and the four older terrain tests |

Two things to know about these gates:

- Reactor-1's `proxy-clippy` stops at the accepted deprecation in `meridian-net`
  and never reaches `meridian-handoff` or `meridian-proxy`. That is why
  `reactor-2-clippy-changed` exists. It adds `-A deprecated` and nothing else, so
  Clippy lints both changed crates with every other warning denied.
- Run 1 failed, and the failure was the gate's, not the patch's. My first version
  of that Clippy gate had no `-A deprecated`, so it stopped at the same
  deprecation. The other gates in run 1 had already passed. Its record is
  [reactor-2-gates-attempt-1.json](data/E30/reactor-2-gates-attempt-1.json).

The record says `internal_sources_match_e28: false`. The one differing file is
`crates/goliath-store/src/main.rs`, which changed in this repository after the
E28 bundle was frozen. Every Meridian source hash matched before patch 13 was
applied. The gate logs stay on the builder's disk under
`.local/evidence/E30/boat-results/`; the record holds their hashes.

## Measurement plan

The reviewer builds one release proxy from reactor-1 plus patch 13 with the E28
settings (thin LTO, one codegen unit, `CARGO_INCREMENTAL=0`), under the host
window lock, and keeps it in `.local/bin/`. The bundle `final-r2-park` is the
frozen `final-r2` bundle with only `meridian` replaced. Tiles, store, sidecars
and bots keep their `final-r2` hashes.

All cells use the existing frontier runner as the r2 staircase did: D9 layout,
`--cpu-mode pin`, `--direct-bots`, 30 s warm-up, 60 s measurement, and its
unchanged validity and pass rules. The proxy-cost numbers come from the same
records: `cells.<n>.proxy` and `cells.<n>.proxy_thread_cpu`.

```sh
python3 -B scripts/e20-frontier --bins .local/bin/goliath-e28/final-r2-park \
  --data-dir docs/experiments/data/E30 --cpu-mode pin --direct-bots \
  --counts 425 --name e30-park-425-r0
```

Step 1, paired proxy cost at 425. Four cells in this order, so drift does not
favour one arm:

| Cell | Bundle | Environment |
| --- | --- | --- |
| `e30-poll-425-r0` | `final-r2-park` | `E30_TERRAIN_POLL=1` |
| `e30-park-425-r0` | `final-r2-park` | none |
| `e30-park-425-r1` | `final-r2-park` | none |
| `e30-poll-425-r1` | `final-r2-park` | `E30_TERRAIN_POLL=1` |

The poll arm is the same binary with the old behaviour, so the pair isolates the
change. The existing `e28-frontier-r2-425-r0` and `r1` cells are the cross-check
that the poll arm matches reactor-1: proxy 0.70 to 0.72 core, managers 0.089 to
0.090. The runner passes its environment to the proxy; check the proxy's
`/proc/<pid>/environ` once in the first cell.

Patch 13 qualifies on cost when all of these hold in both pairs:

- every cell is valid and passes the unchanged frontier rule (all players
  online, 20 TPS, raw tick P99 under 50 ms, no drops);
- summed `terrain-manager` cores fall by at least 0.05 from the poll arm;
- whole-proxy cores fall by at least 0.05 in each pair. This is half of E20's
  0.10 rule. I chose it because the Linux ceiling is 0.079 and the thread
  counters make the manager saving directly visible. A 0.10 rule can't be met on
  this machine at 425 and can be on the Mac;
- proxy context switches fall by at least 15,000 a second;
- the worst tile's raw tick P99 in the park arm is no higher than in the poll
  arm of the same pair by more than 2 ms.

If the manager cores fall but the whole-proxy number does not, report that and
stop. It would mean the saving is inside run-to-run variation, as E20 found for
transport changes.

Step 2, frontier, only if step 1 qualifies. Run 450 and 475 twice each with the
park bundle (`e30-park-450-r0`, `-r1`, `e30-park-475-r0`, `-r1`). The r2
staircase has 425 stable and one failing 450 cell. The frontier moves only if a
count passes twice. I do not expect the frontier to move from this patch alone:
the Linux limit at 450 is the tile tick tail with 1.3 server cores unused, and
the patch frees at most 0.08 core. The reason to run it is the 19,000 wakes a
second that leave the tile cores.

Step 3, crossing behaviour, before the patch enters any selected stack. The
manager decides when readers exist, so run the E12 terrain and border checks and
the E10 1,000-crossing proof on the park bundle, plus one soak cell for proxy
RSS, since scheduler-2 failed there.

Step 4, optional, one cell: `e30-park-425-notiming`, the park bundle with the
runner's `E18_PATH_TIMING` line disabled for that run. It bounds how much of the
proxy cost is the latency histogram. It is a diagnostic, not a frontier cell,
because the runner reads those histograms.

## Unproven

- Any saving. No cell ran. The expected numbers come from per-thread counters of
  the unpatched proxy.
- That the cheapest 383 managers are the interior players.
- Behaviour with a real cluster: no manager thread ran against tiles in the
  gates. The crossing suite in step 3 is what covers it.
- Proxy memory under the patch.
- The Mac. The patch is platform-neutral, and the gates ran on Linux only.
- The cost of the empty `recv` and of path timing. Both are inferred from the
  source and from sample shares.
