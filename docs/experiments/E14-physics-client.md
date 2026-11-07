# E14. A real physics client at a tile wall

2026-11-08. Implementation evidence. Review is separate.

## Verdict

E10 fails before handover because its server never sends the owned border
columns. The fixed E12 build completes 112 actual crossings. Walking passes
the narrow M2 physics-client item on that build, including the visible wall.
Its worst one-second walking loss is 0.158%, with no backward correction.

The broader all-mode momentum line fails. Sprint loses its speed modifier in
the target's attribute packet and settles at walking speed while the sprint
key is still held. Do not treat successful walking as proof that every movement
mode keeps its speed. No milestone box is ticked by this report.

## Method

The client is plain Horizon `fc2868b2b26d56ec76e17414909fe58a3b87bb92`, with
default features, built in the `fast` profile with `CARGO_INCREMENTAL=0`.
`HORIZON_NO_AUDIO=1` disables audio at runtime. Discord and the anonymous
heartbeat are also disabled. No client source or movement code was patched.
`horizon-ctl` launches the client with a fresh game directory and fresh Horizon,
launcher and XDG homes. Its explicit offline connection uses no launcher account
or launcher management command. Prepared 26.3 resources are read only.

The control socket supplies position, motion, frame number and physics tick.
An external sampler polls it and keeps one sample per frame. The client renders
offscreen through Metal at a requested 30 FPS. It is not the headless or scripted
position client. Movement uses held W, Control and Space keys. Creative flight
uses a real double press of Space, confirmed by the outgoing flying ability bit.
Setup teleports happen before measurement, never during the timed walk.

A loopback byte-forwarding observer records decoded protocol 777 movement,
correction, ability, chunk, forget-chunk and view-centre packets. It forwards
bytes before inspecting them. It retains no login profile, account name, UUID,
chat or terrain payload. Both the no-wall control and the crossing use it.

Each server has a separate sampler using non-freezing `player-export` requests.
The retained fields are simulation tick, position, motion and frozen state.
Repeated snapshots of a tick are deduplicated. E2's accepted-input traces and
the proxy's cut/CAS/activation phases provide a second observation. Export closes
menus, but this fixture opens none. Sampling cost is present in both controls.
Frame and server-tick gaps are reported, not silently filled. A frame interval
over 100 ms in either matched one-second window makes its timing unresolved.
Interpolation across such a gap is retained for inspection, not counted as an
exact speed measurement. This is a measurement-quality rule, not a repaired run.

The launcher `scripts/goliath-cluster` is unchanged. E10 uses its retained
binaries. E12 uses the same launcher module with its binary-directory setting
redirected by a thin adapter. E12 executables are copied once and checked against
the hashes in READY. Every run uses that immutable snapshot, even if E12 rebuilds.
No server or proxy source is changed by E14.

E10 starts with edge 2,048. The E12 adapter uses its documented 512-block test
edge. All measured walls meet at the same x=0, z=0 junction. The interior controls
fit inside either layout. These are flat creative fixtures on a shared Mac, not
CPU-isolated capacity results or Java Edition acceptance.

Particle and non-default initial-tile fixtures start another proxy against the
same four backends and sidecars, using only a private generated configuration.
The launcher's original proxy remains idle. The changes select the initial
backend and enable the existing blue dust wall. They change no handover logic.
The extra proxy stops before the cluster does.

### Pass line and units

"Keeps its speed" means at least 95% of the matched no-wall horizontal progress
rate over the one-second interval from 0.5 seconds before crossing to 0.5 seconds
after it. The centre is the first client frame across the geometric wall.
The no-wall path has the same direction, input, initial motion and terrain,
translated 64 blocks into the source tile. Position interpolation supplies the
window endpoints. The rate is horizontal displacement divided by one second,
in blocks per second. Stored motion vectors retain Horizon's blocks-per-tick
units and are not mistaken for displacement speed.

The worst speed dip is the largest loss in that one-second rate against its
matched control. A stall is consecutive physics ticks with no forward progress.
Repeated render frames of one physics tick do not count as stalls. Backward
steps and the largest forward tick step are retained separately. A backward
server correction above 0.1 blocks fails.

Correction packets are compared with the last client-sent position before the
packet arrived. For an absolute correction this is a lower bound on the backward
movement of a continuously advancing client, not an exact handler-time pose.
No correction packet means no such uncertainty. Missing samples cannot turn a
trial into a pass.

A blocked approach has no actual crossing window. Its zero speed in a window
centred on the control's expected crossing time is labelled a nominal window,
never a successful crossing or an invented crossing timestamp.

The E12 acceptance route repeats E4's 14 legs for each of walk, sprint,
sprint-jump and flight. It includes all eight directed cardinal borders and all
four diagonal directions. The route with particles is a separate run.

## E10 defect

The matrix independently starts in each of the four tiles and tries every other
tile with each mode. Both particle settings are covered. Every mode covers all
eight directed cardinal borders and four diagonals.

| Mode | Successful interior controls | Blocked wall approaches | Actual crossings |
| --- | ---: | ---: | ---: |
| Walk | 24 / 24 | 24 / 24 | 0 |
| Sprint | 24 / 24 | 24 / 24 | 0 |
| Sprint-jump | 24 / 24 | 24 / 24 | 0 |
| Creative flight | 24 / 24 | 24 / 24 | 0 |

Each near-wall attempt starts in a missing border column and is stopped after
three seconds without forward progress. Its nominal one-second speed is zero,
against a working interior control. E10 cannot supply 100 actual crossings with
this client. Repeating position packets until the proxy moves it would turn
the test back into a kinematic fixture, so that was not done.

The continuous control walked from `(102, -60, 102)` to past `(26, -60, 102)` in
17.737 seconds. The continuous wall approach started at `(38, -60, 38)`, with
no teleport near the edge. It stopped at `(15.810258, -60, 38)` and remained
there for the final 3.000 seconds of the observed attempt. It never reached
the wall at x=0. Its store owner stayed a, with no handover or correction.

The protocol never delivered the column `(0, 2)` under the stopped player.
The server did send view centre `(0, 2)`. Client pending chunks and every
generation, lighting and meshing queue were zero. The client still reported
`loaded=true` and the status text `Loading terrain...`; that text alone is not
proof of readiness. There were no client warnings explaining the missing column.
The screenshot shows sky and HUD, with no terrain in front of the player.

The plain client's `game/remote.rs` returns before player physics at
`tick_remote_level` when `streaming.is_lit(cx, cz)` is false. A received server
column would be inserted into that lit set by `Streaming::insert_remote`.
The absent owned column is therefore sufficient to explain this freeze.
The client keeps a nonzero old motion vector during the continuous stall, which
is why a motion-only test would have missed it. Position does not advance.

The smallest correction belongs on the server's chunk-delivery readiness path.
An owned FULL column at the closed tile boundary must not wait forever for a
foreign light neighbour that ownership rules deliberately exclude. Keep foreign
simulation and writes fenced. Do not remove the client's missing-column guard
or count stale velocity as movement. E12 owns the fix and its permanent check.

An earlier pilot teleported from the interior control back to `(6, -60, 6)`.
It stayed there for 9,999.914 ms, the worst observed E10 stall. The later continuous approach
shows that this is not merely a teleport-back defect.

## E12 results

The owned-border fix uses engine `d1a0d003...` and proxy `c71386b3...` in immutable
snapshot `cluster-e12-8d82419fbcc0`. Full hashes and READY text are in each raw
result and the snapshot manifest. The pilot received owned column `(0, 0)` and
crossed a to b at 99.9828% of the no-wall speed. It is separate from the two full
56-crossing runs below. Every full-run crossing incremented store generation
exactly once and reached the expected owner. None disconnected.

Speed ratios are percentages of the matched no-wall rate. Each cell gives
minimum / median / maximum. Counts include every actual crossing.

| Mode | Particles off, speed ratio | Particles on, speed ratio | Reliable windows, off / on |
| --- | ---: | ---: | ---: |
| Walk | 99.842 / 100.004 / 100.092 | 99.915 / 99.990 / 100.036 | 14 / 14 |
| Sprint | 93.273 / 93.319 / 98.243 | 93.272 / 94.367 / 95.456 | 14 / 14 |
| Sprint-jump | 98.242 / 98.567 / 99.646 | 98.570 / 98.620 / 99.595 | 8 / 14 |
| Creative flight | 99.972 / 100.009 / 100.041 | 99.948 / 99.998 / 100.047 | 14 / 14 |

All eight directed cardinal borders and all four diagonals occur in each mode
and particle setting. There are 112 actual crossings and 106 reliable matched
speed windows. The six unresolved particle-off jump windows remain in the raw
results. They are not replaced with the better particle-on results.

| Observation | Particles off | Particles on |
| --- | ---: | ---: |
| Actual crossings | 56 / 56 | 56 / 56 |
| Backward crossing corrections | 0 | 0 |
| Backward client position steps in crossing windows | 0 | 0 |
| Observed zero-progress physics-tick stall in reliable windows | 0 ms | 0 ms |
| Worst reliable one-second speed dip | 6.727% | 6.728% |
| Missing frame-number samples over the entire run | 8 of 9,029 | 0 of 9,158 |
| Missing server ticks over the entire run | 2 on c, 0 elsewhere | 0 on all four |
| Largest interval between observed frames over the entire run | 1,170.280 ms | 57.972 ms |
| Largest observed forward step between sampled client ticks in crossing windows | 2.259 blocks | 0.604 blocks |

There is a real display/control-frame hitch in the particle-off run. A
1,170.280 ms interval advances only one observed frame. It also occurs in a
no-wall jump control. A diagonal jump crossing has a 601.682 ms frame interval
inside its crossing window. Its interpolated speed ratio is 53.660%, but the
crossing time is unresolved at that sampling interval, so that is not an exact
46.340% physics-speed loss. The raw frames retain the hitch and catch-up jump.
The largest observed catch-up step is 2.259 blocks in off trial 40. A sampled
tick gap can contain multiple physics ticks; that number is not a teleport.
The current data do not attribute it to the handover, renderer or competing work.
The runs are sequential observations on a shared Mac. The absence of those
hitches in the particle-on run is not evidence that particles improve performance.

The particle-on run has no lost frame-number or server-tick samples. All 56
matched windows are resolved, including every jump and flight direction.
Its repeatable sprint dip therefore does not depend on the earlier hitch.
Its largest forward-step excess over the matched control is 0.00264 blocks.

The source accepted several inputs clamped at x=0.301 while the real client
continued forward. Its accepted-input motion had zero horizontal components.
That does not prove a client stall. In walking, the client's positions and motion
keep advancing, and no arrival position packet resets them. The clamp is visible
in the server samples, but not as a walking freeze on the client once terrain is
present.

### Sprint defect

`sprint-on-trace.json` extracts particle-on trial 15, b to c. The source sends
movement-speed attribute 26 with base `0.10000000149` and modifier
`minecraft:sprinting`, amount `0.30000001192`, operation 2. At 175.641 ms after
the client crosses, the target sends the same attribute base with an empty
modifier list. W and Control remain held throughout.

The plain client's attribute handler removes the old modifiers before applying
the packet. Its motion drops from `0.1532168` blocks/tick to about `0.1178603`,
the walking value, with no position or velocity correction packet. The settled
loss is 23.08%. The one-second crossing metric sees a smaller loss because only
the latter part of that window follows target activation. Twenty-six of the
28 sprint trials miss the 95% one-second line. The other two do not establish
that sustained sprint speed was preserved.

This is not a client physics fix. Preserve the source's live sprint flag and
transient movement attribute through activation. Build the self-attribute delta
from that post-activation state, rather than the held login's default attributes.
The smallest regression is W plus Control held through a crossing, with the
same movement modifier before and after the target delta. Check both the
one-second window and the following second.

The plain code paths are `game/remote.rs` at `UpdateAttributes`,
`lan/attributes.rs::apply`, and `motion.rs::set_sprint_modifier`. They explain why
a sprint flag can remain set while the speed modifier has been removed. The
packet and tick evidence, not that source observation alone, establish this run.

### Visible wall

The particle-on crossing run received 23,400 particle packets. A separate
stationary capture at `(2, -60, 6)`, looking towards x=0, received 288 packets
and visibly renders the blue dust wall. Its raw result is `e12-wall-render.json.gz`.
The original moving screenshot was taken after passing the wall and did not
show the particles. It was discarded, not presented as rendered proof.

![Blue particle wall rendered by the real Horizon client](data/E14/e12-wall-visible.png)

## Commands and failures kept

```sh
python3 -B scripts/e14-checkout
git -C .local/src/horizon-tiles stash push --include-untracked -m e14-plain-client
CARGO_INCREMENTAL=0 cargo build --locked --profile fast \
  -p horizon-client --bin horizon-client -p horizon-client-lib --bin horizon-ctl
python3 -B scripts/e14-physics --name pilot-1
python3 -B scripts/e14-physics --name pilot-2
python3 -B scripts/e14-physics --name e10-continuous-approach --approach
python3 -B scripts/e14-matrix
python3 -B scripts/e14-snapshot
python3 -B scripts/e14-physics --name e12-fixed-full-off --variant e12 \
  --snapshot .local/bin/horizon-e14-client/cluster-e12-8d82419fbcc0 --suite
python3 -B scripts/e14-physics --name e12-fixed-full-on --variant e12 \
  --snapshot .local/bin/horizon-e14-client/cluster-e12-8d82419fbcc0 --suite --particles
python3 -B scripts/e14-physics --name e12-wall-render --variant e12 \
  --snapshot .local/bin/horizon-e14-client/cluster-e12-8d82419fbcc0 --particles --render-only
python3 -B scripts/e14-runner-test
python3 -B scripts/e14-analyse
python3 -B scripts/e14-restore
python3 -B scripts/e14-verify
git diff --check
scripts/clean-builds
```

The Cargo command runs inside the assigned Horizon-tiles checkout. Binaries are
retained under `.local/bin/horizon-e14-client/`. Build completed in 3 minutes
7 seconds. Only its one existing target directory was used.

Before stashing, a temporary clean index applied the four saved E7/E11 patches
to the exact pinned commit. Every changed and untracked file matched byte for
byte, 25 files in total. Two intent-to-add entries made the first stash fail.
Their markers were recorded and removed from the index, leaving their files
intact, before the successful named stash. `e14-restore` ran `stash pop --index`
and restored both intent-to-add markers. The original status, complete HEAD
diff and staged diff have identical SHA-256 hashes. All 25 source files are
byte-identical to before. `restoration.json` records the check. The stash was
dropped by the successful pop. No E14 source edit remains in that checkout.

`pilot-1` failed setup because integer console teleport coordinates were centred
at `.5`. The fixture now supplies explicit decimal coordinates. That pilot
contains no crossing result. `pilot-2` exposes the real missing-column failure.
Quiet-window interruptions have their own retained result files. Repeated runs
use new names, never replace raw evidence. All log parsing stops at the last
complete line.

Ten analyser/observer tests pass, including planted stalls, backward corrections,
missing frames, missing server ticks, incomplete windows and retained failed
attempts, the sprint attribute codec and unresolved native frame gaps. No full
upstream or root Rust gate is claimed from this experiment.

## Verification, cleanup and limits

The independent evidence checker passes. It recomputes the two 56-leg route and
owner chains, checks every generation increment, all 12 directed tile pairs per
mode, all 96 E10 blocked approaches and 96 successful controls, matching E12
binary hashes, resource-manifest equality, exact checkout restoration and JSON
privacy. This is an evidence-integrity pass, not a sprint-speed pass.

Raw and derived E14 evidence totals about 5.8 MiB. Every raw run, including the
setup failure, missing-column pilots and quiet-window interruption, remains.
Three small PNGs are kept. Two redundant moving screenshots were discarded.
No result JSON contains a real home path, fixture account name or UUID. A final
metadata scan found none of the 28,180 prepared resource files modified after
the client binary was retained.

Every E14 world, log and private-home directory was deleted after extraction.
No E14 game or cluster process remains. Plain `scripts/clean-builds` exited 0,
freed 0 MiB. The first cleanup reported the shared project at 9,953 MiB. The
final cleanup reported 9,887 MiB, with root 373 MiB, assigned Horizon-tiles
3,620 MiB and Meridian 502 MiB under the cap. It correctly skipped the other
agent's active Horizon target at 2,344 MiB. No `--all` cleanup ran.
These are end samples, not continuous peak measurements.

`git diff --check` and all ten Python tests pass. The client build passed.
Full upstream and root Rust gates are not claimed because E14 changed no Rust
source, and other agents own the root Rust changes. Java Edition, vehicles,
crashes, leases, capacity and newer E12 binary revisions are not accepted here.
The remaining native-frame hitch needs profiling. The sprint fix needs its own
repeat of this physics matrix after E12 implements it.

No commit, push, upstream action, Folia/Paper/GPL source read or launcher
management command was made.

## Screenshots

The PNG screenshots show rendered game textures, which are Mojang's, so they are kept out of the public repository. They stay local under `.local/evidence/E14/`. The packet counts and frame data above are the evidence; the screenshots only illustrate them.
