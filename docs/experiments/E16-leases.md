# E16. Time-bounded tile authority

Implementation evidence. Review and commits are separate. No Folia,
Paper, GPL source or excluded research was read. Nothing was committed, pushed
or sent to GitHub by this agent. Horizon and Meridian belong to E12 and were not
edited here.

## Result

The store grants durable leases and rejects expired holders even when their fence
still matches. A caller-driven `LeaseKeeper` gives tile processes a local deadline
without store contact. A forced takeover stops renewals, then waits for the old
grant to expire. Store restart conservatively starts a full-L wait again.

The additional partition simulator passed 20,000 seeds at each of three lease
lengths. All 60,000 controls that ignored the local deadline failed. Existing E1
and E9 sweep outputs still match byte for byte. The short suite passes 52 tests,
including six new real-process store tests, in under 30 seconds.

The quiet 64-client latency check passed all six rows. Legacy commit P95 was
16.054 to 16.071 ms. With active leases and heartbeat traffic it was 15.954 to
16.168 ms. E15's under-20-ms line is preserved in this workload.

Recommend L = 1,000 ms, a 100 ms client margin and a renewal every 250 ms. This is
a starting operating choice, not a measured host clock bound. The store-only
takeover measurements and E13's restart cost are compared below.

## Contract for E12

The store re-exports `LeaseKeeper`, `LeaseRequest` and `LeaseGrant` from the model.
No library thread, runtime or timer renews them. E12 owns scheduling and shutdown.

| Call or operation | Meaning |
| --- | --- |
| `Client::claim_lease(tile, lease_ms, margin)` | Fresh claim and original-send ticket. Returns a keeper only if the real reply arrives in its usable window. |
| `Client::renew_lease(&mut keeper)` | Fresh, increasing renewal counter. True installs the new local deadline. False permanently stops the keeper. An IO error leaves its old deadline unchanged. |
| `keeper.may_act(Instant::now())` | The sole local authority gate. False before a grant, at the deadline, and forever after an observed expiry or `stop()`. |
| `keeper.deadline()` | The current local deadline, never a store timestamp. |
| `keeper.fence()` | The claimed fence. Presence alone does not mean it is still authorized. |
| `Operation::ClaimLease { tile, lease_ms }` | Grants fence plus L, or fails with `LeaseBusy`. L is 1 through 60,000 ms. |
| `Operation::RenewLease { fence, counter }` | Extends a live, current, non-draining lease. Counter must increase. |
| `Operation::BeginTakeover(fence)` | Durably disables further renewals for that exact current fence. It does not grant the replacement authority. |
| `Operation::CheckLease(fence)` | Checks current, active, unexpired authority for placement or activation. It does not renew a keeper. |
| `Response::Leased(LeaseGrant { fence, lease_ms, counter })` | Counter zero is the original claim. Renewals return their accepted counter. |

E12 must gate each actual cut append, flush and install, player action, explicit
import placement and ordinary returning login. Gate loaded saved generations
too. Stop authority exactly when `may_act` becomes false, even if the store is
unreachable. Cancel queued work. A check when capturing or queueing a cut is not
an append check. Do not publish a successful cut receipt after expiry.

Critical work must finish before the local deadline. A process that can pause
without bound between its check and the physical side effect cannot meet this
contract with a lease alone. It needs a storage-side fence or a bounded critical
section. A store lookup before append still leaves E13-1's race.

Do not hold the keeper's action-path lock across blocking store IO. The helpers
are convenient for a single caller, not permission to block all expiry checks
behind a network call. For a separate heartbeat worker, take a ticket under a
short lock, perform IO outside it, then apply the response under a short lock.

The explicit driver API is `LeaseKeeper::new(tile, lease_ms, margin)`,
`request(now)` and `observe(ticket, grant, now)`. Call `request` before sending a
fresh command. Counter zero goes with `ClaimLease`. Later `ticket.counter()`
values go with `RenewLease`. Keep the same complete command, request ID and ticket
for every transport retry. Never give a retry a new send time. The keeper rejects
superseded tickets, duplicate replies, mismatched fences, lengths or counters,
and replies that arrive after its previous deadline. Retire an expired keeper.
A new keeper requires a fresh claim request identity, not a cached claim reply.

### Deadline and clock inequality

Let s be the real send time of the original request and g the store's grant time.
The store cannot grant before send, so g >= s. The local deadline is the client's
monotonic time at s, plus L minus margin. It is not response-receive time plus L.
Queueing, request delay, a slow barrier and a delayed reply consume usable time.
They never give the client more authority. A too-late reply is discarded.

Let rc bound the fractional rate error of the client clock and rs the store's.
The latest real local expiry is s + (L - margin) / (1 - rc). The earliest store
expiry is g + L / (1 + rs). The exact sufficient condition is:

```text
s + (L - margin) / (1 - rc) <= g + L / (1 + rs)
margin >= L * (rc + rs) / (1 + rs), since g >= s
```

At L = 1,000 ms and rc = rs = 0.001, the minimum drift margin is about 1.998 ms.
The proposed 100 ms margin gives local expiry no later than s + 900.901 ms and
store expiry no earlier than g + 999.001 ms. There is at least 98.100 ms of real
slack at zero request delay. Delaying a grant increases that slack.

These are assumptions about advancing clocks, not a wall-clock synchronization
requirement. No lease time comes from `SystemTime` or the cluster sky clock.
Do not carry a keeper across host suspension or a monotonic clock anomaly.
Stop it before processing resumed work, then claim anew. The clock-rate bound,
suspend handling and critical-section bound need E12 and deployment verification.
The margin does not solve an unbounded pause after the final authority check.

## Store ordering, persistence and compatibility

The serialized engine checks leases at each command's preparation point, before
retry receipts. Both CAS fences must be current and live. Integer and UUID create
and commit, clock writes, release, renewal and activation checks all use the same
lease-aware authority. A receipt cannot renew a lease or bypass expiry. Ordinary
read-only player, clock and listing replies contain data, not authority.
Directory encoding is unchanged, but an expired lease reports `active = false`.

The last accepted renewal sets the store's monotonic deadline to grant time + L.
An identical request retry returns its old grant without extending that deadline.
A reused or out-of-order counter under a fresh request ID fails `RenewalReplay`.
A delayed higher-counter request can extend only a lease that is still live and
not draining. A request arriving after expiry cannot revive it.

`BeginTakeover` and renewal share the engine's command order. If renewal wins
first, takeover waits that later grant + L. If takeover wins first, renewal fails
`LeaseDraining`. The old holder may still act until its local deadline. The
replacement retries `ClaimLease` with no early activation. `Release` does not
shorten the retained lease either. There is no unsafe immediate-force escape.

Lease length, renewal counter, drain state and fence changes use the existing
checksummed journal and full barrier before replies. Snapshot extensions retain
those values. No monotonic timestamp is serialized. Historical journal commands
replay in order without applying today's expiry checks. After recovery IO, every
durable lease gets a new store-local deadline of now + L, including released or
previously expired leases. This can delay availability after a restart, but
cannot shorten a grant. Repeated restarts can delay takeover repeatedly.

Wire version 1 and tags 1 through 14 are unchanged. New operation tags are 15 for
ClaimLease, 16 for RenewLease, 17 for BeginTakeover and 18 for CheckLease. Response
tag 8 contains fence, L and counter. New fault codes 7 through 11 are LeaseBusy,
LeaseExpired, LeaseRequired, RenewalReplay and LeaseDraining. Old unleased traffic
still works. Old snapshots without the lease extension still load.

Once a tile uses leases, legacy Claim and Heartbeat fail `LeaseRequired`. They
cannot bypass its wait or silently renew it. Switching a live legacy holder to
leases fails `LeaseBusy`, because that old binary has no local deadline. Stop and
release all legacy holders before enabling E16. Compatibility is not a claim that
an unmodified E10 tile becomes safe merely by using the new store executable.

## Partition model

Reproduce with:

```sh
just leases 20000 docs/experiments/data/E16/leases.csv
```

This adds `goliath-sim leases <seeds> <csv>` beside the existing commands. It uses
the production `StoreLease` and `LeaseKeeper` kernels with separate synthetic
monotonic clock rates between -1,000 and +1,000 ppm. Seeds vary request and reply
delays, last-renewal time and a store restart during the partition. It never waits
for real time. Results are deterministic and contain no absolute timestamps.

P5 captures a coherent chest/inventory change and flushes its cut. The old process
then captures a queued cut, loses store contact and keeps running. A forced drain
blocks future renewal. The replacement waits for store expiry, claims a new fence
and recovers the same durable image. The old process tries queued and fresh cuts,
flush, install and a returning login before and after takeover. Its append check
gets only its own old fence, as a local FENCE file would. It never receives the
current store fence as an oracle.

The independent checker counts old-owner writes and simultaneous authoritative
players and item identities across both process copies. With enforcement, the old
process can still run ticks but cannot cross its local gate. The control bypasses
only that gate. Final replacement recovery also checks missing identities.

| L and margin ms | Seeds per mode | Lease failures | Ignored-deadline failures | Stale appends in control | Duplicate identity observations in control |
| --- | ---: | ---: | ---: | ---: | ---: |
| 500 / 50 | 20,000 | 0 | 20,000 | 80,000 | 480,000 |
| 1,000 / 100 | 20,000 | 0 | 20,000 | 80,000 | 480,000 |
| 2,000 / 200 | 20,000 | 0 | 20,000 | 80,000 | 480,000 |

Each enforced row has zero stale append, flush, install, login, overlapping-owner,
duplicate and missing-identity observations. Each control row has 80,000 stale
observations at each of the four named operations. Counts are repeated checker
observations, not that many distinct lost or duplicated items. About half the
seeds restart the store. These are 120,000 additional bounded scenario-seeds,
not a replacement for E9's original random crash schedule or a proof of unbounded
distributed safety.

## Real-process tests

The six short tests start actual store executables with full barriers, loopback
TCP, journal rotation and fresh folders under `.local/runs/`.

- Expiry without replacement rejects integer and UUID create and CAS, retries of
  successful requests, sky writes, release, renewal, takeover and activation.
  The fence still matches. Directory entries become inactive. Legacy Claim is
  refused and a fresh leased claim advances the fence.
- An identical renewal replay does not extend the wait. A fresh request with an
  equal or lower counter fails. Replacement succeeds on the original grant's
  schedule rather than the replay's schedule.
- A TCP proxy delays the actual renewal request by 150 ms, then holds the real
  response past the old local deadline. The keeper rejects it and cannot resume.
  Wall-clock inputs step to the epoch and one day ahead without changing its
  monotonic expiry. This does not alter the machine's wall clock.
- A SIGKILL restart mid-lease waits a full L from recovery. Both journal-only and
  compacted snapshot paths retain the drain and reject another renewal.
- Forced takeover and release both wait out the last renewal, not the first
  claim. Neither can authorize an early replacement.
- The blocking helpers claim and renew, then permanently stop their keeper when
  a takeover prevents renewal.

Four model tests cover exact deadline equality, original-send anchoring, reply
reordering, duplicate delivery, replaced fences, full-L recovery, invalid lengths
and margins, wall-clock independence and the drift inequality. Once `may_act`
observes expiry it latches closed, even if a caller later supplies an older time.
The short simulator test runs 1,000 seeds per L in both modes and requires working
controls. No test depends on the long timing experiment.

## L and recovery cost

`crates/goliath-store/experiments/e16.py takeover` ran 18 store-process trials.
Each grants and renews a lease, stops heartbeats, forces a drain and probes until
the real store accepts a replacement. Three trials per L fail immediately after
renewal or one quarter of L later. Three more restart the store mid-lease. All
claims use full barriers. Probe spacing is 5 ms plus the real request cost.

| L ms | No store restart, wait from failure ms | Store restart, wait from recovery ms |
| ---: | ---: | ---: |
| 500 | 375.104 to 504.239 | 506.285 to 506.834 |
| 1,000 | 742.497 to 1,002.696 | 1,008.349 to 1,012.439 |
| 2,000 | 1,496.495 to 2,007.681 | 2,006.433 to 2,009.117 |

The data are in [data/E16/takeover.json](data/E16/takeover.json). The wait includes
probe and durable claim overhead. The last renewal's request-send time is also
recorded, so a quick claim cannot be mislabeled as waiting out L. These measure
the store gate, not a full Horizon reconnect.
The final table uses the same retained executable as the latency runs. An earlier
18-trial run remains in `takeover-initial.json`, with its own executable hash.

E13 measured a 0.589 s replacement without a lease. With heartbeat period L/4,
a healthy crashed process usually leaves 0.75 L through L of unexpired authority.
If replacement startup overlaps that wait, the lower bound is approximately
max(0.589 s, remaining lease). If startup waits until after the claim, add the two
instead. At L = 1 s, those estimates are about 0.75 to 1.01 s overlapped or 1.34
to 1.60 s serialized. E12 must measure its actual startup order.

L = 500 ms could mostly hide behind E13's restart cost, but a 50 ms margin and
125 ms renewal cadence leave less room for scheduler stalls and slow storage.
L = 2 s roughly doubles the failure wait for resilience not needed by the
observed loopback commit timings. Start with L = 1 s and margin = 100 ms. Renew
every 250 ms and budget the network call well inside the remaining local window.
Four tiles then need about 16 renewal requests per second. Do not increase L
to hide a stuck worker. Monitor failed renewals and stopped keepers.

## Latency and verification

The reviewer first authorized E12 and E14 acknowledgements, then confirmed E14 had
finished and authorized proceeding with E12 alone. The first wait was interrupted
before any sample, and its markers were released. The E16 runner's final request
requires E12's acknowledgement and one-minute load at or below 4. It waits at most
30 minutes, monitors eligibility every 20 ms through the
existing E15 harness, and removes its request and acknowledgements in cleanup.
It runs the same Rust driver at 64 clients, 128 samples each, 256-byte blobs,
compaction every 256 mutations and F_FULLFSYNC. Three legacy rows check the
unchanged workload. Three leased rows add two L = 1 s leases renewed every 250 ms.
The lease renewals are control traffic, not extra timed commit samples.

All six quiet rows passed. Each contains all 8,192 commits without trimming.
Percentiles use nearest rank, as in E15.

| Mode | Run | P50 ms | P95 ms | P99 ms | Peak one-minute load |
| --- | ---: | ---: | ---: | ---: | ---: |
| Legacy | 1 | 11.930 | 16.071 | 16.878 | 1.261 |
| Legacy | 2 | 11.031 | 16.066 | 17.337 | 1.261 |
| Legacy | 3 | 11.005 | 16.054 | 16.962 | 1.261 |
| Leased | 1 | 11.010 | 15.954 | 16.810 | 1.160 |
| Leased | 2 | 11.805 | 16.168 | 17.088 | 1.160 |
| Leased | 3 | 11.998 | 16.067 | 16.985 | 1.227 |

The final request ran from 00:50:48.742 through 00:51:22.046 local time on
2026-11-08, including its acknowledgement wait. Both request and E12 acknowledgement
remained present through every sample. No row was disturbed. All measured Rust
connect phases were zero because handles were warmed. The largest call was
23.517 ms in legacy run 2. This is a P95 criterion, not a per-call 20 ms bound.
The maximum leased-row P95 is 0.097 ms above the maximum legacy-row P95, well
inside the observed E15 range. This is not a general bound on lease overhead.

Raw CSV and per-run metadata are in [data/E16/latency/](data/E16/latency/).
The combined [quiet summary](data/E16/latency/quiet-summary.json) records all six
rows. The runner uses the existing E15 service, sampling and Rust driver path.
Only leased setup and two real heartbeat streams are added for the leased mode.
Those renewal streams are driven by the experiment process, not by library
threads. They do not replace E12's own keeper and expiry integration tests.

Commands run from the repository root include:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
/usr/bin/time -p just test
cargo test --workspace --all-features
cargo build --release --workspace
just leases 20000 docs/experiments/data/E16/leases.csv
target/release/goliath-sim sweep 66667 .local/runs/e16-e9-reproduction.csv
cmp .local/runs/e16-e9-reproduction.csv docs/experiments/data/E9/sweep.csv
target/release/goliath-sim sweep 66667 .local/runs/e16-e1-reproduction.csv 0
cmp <(sed '/^P5,/d' .local/runs/e16-e1-reproduction.csv) docs/experiments/data/E1-results.csv
python3 -B crates/goliath-store/experiments/e16.py takeover --trials 3
python3 -B crates/goliath-store/experiments/e16.py quiet-latency \
  --quiet-agents E12 \
  --binary .local/bin/goliath-store-e16-7b413bd/goliath-store \
  --driver .local/bin/goliath-store-e16-7b413bd/goliath-store-load \
  --output docs/experiments/data/E16/latency --trials 3
git diff --check
scripts/clean-builds
```

The default E9 reproduction exits 1 because delayed-abort P3 fails, as before.
Its CSV still matches SHA-256 `0c92e6ed296670ca0fb29e8498380583c149a6da976a4c8522b9e34bf74e9cbd`.
The instant-abort reproduction exits 0 and the non-P5 CSV matches E1 SHA-256
`e70e7e27b0bdd11566b590166a137a59df3b71d3f250172f7cd0de68150c66b7`.
Initial Clippy found a constant-only test assertion. It now checks the keeper's
computed deadline instead. No lint suppression was added.

Final formatting, all-feature Clippy, normal and all-feature tests, release
workspace build and diff checks passed. The final timed `just test` was 5.71 s,
including Cargo and doctests, with 52 tests. The architecture audit found no
missing ownership docs, kind-named modules, wildcard imports, new unsafe or lint
suppressions. Library code still owns no threads. Independent recomputation of
all six CSV P95 values, counts and quiet eligibility passed, covering 49,152
timed commits. No home path or em dash appears in the retained E16 results.

## Limits and cleanup

This does not yet repair or retest the real Horizon cut writer or returning-login
paths. E12 must wire every gate and rerun both E13 reproductions. No Minecraft
client gap, full leased-cluster restart, deployment, disk power cut or clock-rate
bound was measured. SIGKILL tests prove process recovery, not controller barriers.
The wall-clock checks use independent test inputs, not an OS clock change.

The lease model assumes advancing bounded-rate clocks and actors that obey the
deadline. The ignored-deadline control shows why this assumption matters. A
malicious process or an unbounded check-to-write pause needs a physical storage
fence. The simulator uses the existing P5 image and cut log; it does not model
Horizon byte encoding or every interleaving of an asynchronous write pipeline.

The pre-existing unbounded receipt retention and single-store availability limits
remain. Restart conservatively blocks even a lease that had already expired;
there is no fast recovery based on a wall-clock guess. Downgrading the store
after enabling new operations is unsupported.

All experiment worlds and store logs created here live under `.local/runs/` and
are removed after extraction. Only small results remain in `data/E16/`. No result
contains a home path. The retained E16 store and Rust driver are under
`.local/bin/goliath-store-e16-7b413bd/`, where `7b413bd` names the starting commit,
not a committed E16 revision. The store SHA-256 is
`6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af`.
The driver SHA-256 is
`a51e55c0763ee42c416ab835756ab1d2b8c49f1cb0312641fac4dbc1bd6cbe58`.
Final cleanup is plain `scripts/clean-builds`, never `--all`.
It completed successfully, freed 0 MiB and reported the workspace target at
415 MiB, all targets under their caps and the shared project at 10,000 MiB.
Raw E16 evidence is about 1.2 MiB, including the earlier timing run. Temporary
reproduction CSVs were removed after their exact comparisons. Their source data
and reproduction commands remain. The reviewer's later quiet request was honored
with an E16 acknowledgement; that acknowledgement was removed when it ended.

## Independent verification

The reviewer checked this on 2026-11-08. Formatting, all-feature Clippy and 52 tests passed. `goliath-sim leases 20000` reproduced `data/E16/leases.csv` byte for byte, SHA-256 `74eae099...`: zero failures with enforced leases at every length, and 20,000 of 20,000 failures for the control that ignores its deadline. The default sweep still matches `0c92e6ed...` and E1's non-P5 rows still match `e70e7e27...`. The 64-client timing wasn't rerun separately; the reviewer's own reruns of E15's binary in the same session gave 16.1 to 17.0 ms. The Horizon side, wired by E12, is what M3 will measure.
