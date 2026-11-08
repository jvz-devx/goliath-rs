# E20. Profile and next-build proposal

No new production edit or build is proposed as an automatic next step.
Meridian remains read-only at scheduler-2. The official frontier remains the
reproduced reactor-1 result, 650 players, 2.0 times the matched baseline.

## Two combined pilots, not a new frontier

Both rows use combined-1, the unchanged D9 layout and four-core server cap.
All players remain online, with no bot lateness, dropped raw ticks or outside
noise violation. No lower bracket was run after these rows.

| 650 players | Result | Server cores | Proxy cores | Tile-a P99 ms | Maximum tile P99 ms | Server STOP cycles / wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| r1 | fail | 3.1743 | 1.0366 | 51.0471 | 51.0471 | 306 / 2.4765 |
| r2 | pass | 2.9925 | 0.8939 | 27.6346 | 39.5042 | 127 / 0.9466 |

Peak threads fall from about 1,330 to 680. That is a real thread-count cut,
not proof of a repeatable capacity improvement. The two proxy costs differ
by 0.143 core despite matching layout and approximately 41.6 million
clientbound packet timing samples. This variation is larger than several
plausible micro-optimizations.

## CPU profile

`combined1-profile650-1` is a perturbed diagnostic. It includes five-second
process samples, an eight-second proxy Time Profiler capture, native tile-a
slow-tick spans and controller STOP/CONT timestamps. It is excluded from
capacity acceptance. Full traces and sampled stacks stay under
`.local/evidence/E20/combined1-profile650-1/`.

The exported Running samples total 7.872 CPU seconds across an 8.551-second
sample range. Blocked stack occupancy is not used as CPU share. Missing
unwinds are 0.064 percent of Running weight.

| Thread role | Target Running CPU weight | Top leaf frames |
| --- | ---: | --- |
| Four IO drivers | 88.73% | `__recvfrom`, `writev`, `kevent`, ghost `View::apply`, `mach_absolute_time`, allocator/free |
| Four shared control workers | 6.90% | `__psynch_mutexwait`, `__psynch_mutexdrop`, comparison/allocator, terrain reconciliation |
| Short-lived wall workers | 4.15% | `write`, SHA-256, connect/receive, thread lifecycle |
| Other target threads | 0.22% | connection and lifecycle work |

Within the IO-driver role, reads are 28.56 percent, writes 22.92 percent,
allocation/copies 13.09 percent and selectors 5.24 percent. Locks/wakeups are
only 0.33 percent. The read samples do not distinguish successful reads from
WouldBlock returns. No failed-read count is claimed.

The shared workers still spend CPU on their receiver mutex: the sampled
mutex wait/drop leaves account for 46.78 percent of that small role. Their
whole native counter cost is about 0.064 to 0.073 core at 650. Eliminating
every shared-worker operation cannot save more than that amount, and real
control/recenter work must remain. Manager wakeups are not the main remaining
IO-driver cost.

E21's earlier 0.272-core contained ON/OFF proxy difference covers the entire
terrain feature in reactor-1, including its per-player managers. It must not
be treated as an independent socket-reader saving on top of the manager work
already removed by scheduler-2.

## Tile-a tail

The old unperturbed rows retain tick sequence, game time and duration only.
They have neither absolute tick timestamps nor work phases. Their exact
51.047-ms P99 tick and 73.594-ms maximum cannot be assigned retrospectively
to a sampled stack. Limiter pauses are never subtracted from those values.

The existing `HORIZON_SPIKES=20` option supplies new native wall/CPU/span
evidence without rebuilding the engine. Seventy-four slow-tick lines were
retained during profiling. Spans are inclusive and nested, and may include
named between-tick work, so they must not be summed as independent costs.

| Captured native tick | Main-thread CPU | Leading spans |
| ---: | ---: | --- |
| 46.3 ms | 17.8 ms | `tick_lan` 42.4/13.9 ms, `lan_guests` 34.8/6.4 ms |
| 32.5 ms | 14.5 ms | `tick_lan` 28.8/11.2 ms, `lan_guests` 24.4/6.8 ms |
| 32.4 ms | 16.2 ms | `tick_lan` 19.0/12.2 ms, packet/drain 11.3/2.0 ms |
| 31.0 ms | 25.0 ms | `tick_lan` 24.6/18.8 ms, `lan_guests` 18.8/13.0 ms |

The 46.3-ms tick overlaps the direct tile-sampling boundary. The next three
listed ticks are outside direct tile sampling, but the proxy profiler and
shared limiter can still affect them. The long wall tails include substantial
off-CPU time; they are not evidence of 46 ms of simulation CPU. One-second log
timestamps prevent exact per-tick attribution to individual STOP intervals.
The profile did not reproduce the old tile-a P99 failure.

The tile main-thread sample's leading nonwaiting leaf is
`Waypoints::update_or_create`, followed by `write` and memory movement.
Static inspection finds a linear source lookup in each receiver's connection
vector. This is a useful separate optimization lead, not proof that the old
slow tick was caused by waypoint CPU. A general waypoint optimization would
also improve plain Horizon and would require a matched baseline rerun.

## Proposal and expected saving

Recommended order:

1. Keep Meridian unchanged while E22 validates the sparse observer and shared
   terrain-view design. Those changes target redundant reader work directly.
   A new host build is not justified merely by the thread-count reduction.
2. If a transport experiment is authorized, test one bounded extra readiness
   drain before write flushing, with no batching timer. Preserve FIFO, cipher
   state, byte/chunk caps, partial-write offsets and physical completion clocks.
   Add read/write/WouldBlock and vectors-per-write counters so the premise can
   be measured rather than inferred from stack samples.
3. A separate bounded buffer-reuse experiment may reduce allocation churn.
   It must reuse only byte storage, not share codec/cipher state between clients.

At the observed proxy cost, socket writes are roughly 0.19 core of sampled
work. A 25-to-35-percent reduction would save about 0.05 to 0.07 core before
extra selector cost. A defensible net expectation is about 0.04 to 0.08 core
at 650, not a promised capacity increase. Manager no-op suppression alone has
an approximately 0.07-core absolute ceiling and a smaller attainable saving.
The two targets overlap and must not be added as independent guaranteed wins.

This expected transport saving is below the observed pilot variation. I
recommend deferring another standalone micro-optimization build until E22's
reader architecture is measured, unless the reviewer specifically authorizes
the bounded experiment. If authorized, use a fixed paired ablation order and
require a reproducible reduction of at least 0.10 proxy core, with no worse
tile tail, before spending time on a new full frontier campaign. No packet,
tick, physics or CPU pass line is relaxed.

E20 needs no immediate Meridian edit. The checkout can be handed over after
the reviewer confirms the next owner; its current byte identity is the
scheduler-2 source manifest. No source handoff is implied by a host build slot.

## Frozen evidence and reproduction

- Frozen input: `.local/bin/meridian-e20/combined-1`.
- Proxy SHA-256: `8716c34f6e799e2ddce31aff153b86cf9c78408bfcd64b4c78551851d2b399ad`.
- Engine SHA-256: `ea4537868c90771e0d030ccfff1dbe36dace215a2c70d652dd86b38b84434090`.
- Native profile: `data/E20/combined1-profile650-1-profile.json`.
- Running weights and native spikes: `data/E20/combined1-profile650-1-analysis.json`.
- Sample occupancy: `data/E20/combined1-profile650-1-proxy-sample-summary.json`
  and `data/E20/combined1-profile650-1-tile-sample-summary.json`.

```sh
python3 -B scripts/e20-deep-profile --name combined1-profile650-repro --bins .local/bin/meridian-e20/combined-1 --no-other-agents --e22-peer
python3 -B scripts/e20-profile-analysis --name combined1-profile650-repro
```

Use a fresh name. The first command creates and releases a quiet request and
waits for E22's acknowledgement. The E21 waiver applies only while the reviewer
confirms E21 remains idle. Profiling is deliberately separate from acceptance.
