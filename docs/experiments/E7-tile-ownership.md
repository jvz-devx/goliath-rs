# E7. Tile ownership

Implementation and measurements for M1. Review is still required.
No milestone box is ticked. Nothing was committed or sent to GitHub.

## Configuration and patch

`patches/horizon-02-tile-ownership.patch` applies to Horizon
`fc2868b2b26d56ec76e17414909fe58a3b87bb92` after
`e2-horizon-input-trace.patch`. It is off without `HORIZON_TILE`.

```sh
HORIZON_TILE=origin_x,origin_z,edge,index_x,index_z
```

Coordinates and edge are blocks. The origin and edge must align to 512 blocks.
The edge must be positive. Overflow is rejected. Indices can be negative.
The owner is the half-open square from `origin + index * edge` to that point
plus `edge`. This gives the same Euclidean geometry as `TileGeometry`.
`HORIZON_TILE_TEST_SMALL=1` permits 16-block alignment for test worlds only.
The same mask applies to every dimension opened by that process.

D8 was recorded while E7 was running. Its 2,048-block production edge is valid
here. Its unsharded Nether and End processes must run without `HORIZON_TILE`.
An Overworld backend still applies its square if it opens another dimension
locally. D8 portal routing to separate dimension processes is not implemented or
verified by this patch.

The breach fixture owns `[-64,64)` on both axes. The production-aligned read
and timing fixtures own `[-512,512)` on both axes. A saved world's spawn must
be inside its tile. A tile elsewhere needs a world seeded with that spawn.

The patch changes these rules:

- All `Levels` predicates return not ticking for a foreign chunk, including
  the unrestricted fixture case with no ticket map.
- `World::set`, `ChunkMap::insert`, `get_mut` and direct light writes reject
  foreign chunks. The private map cannot gain a foreign column through an
  iterator. Rejections increment a counter and log counts 1, 2, 4 and so on.
  A debug build asserts at the offending storage writer. The fast build refuses
  the write without panicking. Ordinary commands often reject an unloaded
  position before reaching this counter.
- Missing simulation chunks read as bedrock. This includes unloaded owned
  chunks, foreign chunks, and foreign space above or below the build height.
  Collision scans and palette shortcuts see solid unknown space. Unknown light
  reads zero. Full lighting uses solid virtual columns, and incremental lighting
  uses a dark solid section. These columns are never resident or saved.
- Generation candidates, including forced-chunk candidates, stop at the wall.
  The outer two owned chunk rings skip decoration. Their finishing and lighting
  do not wait for foreign chunks.
- A piston move is refused if its head, pushed destinations or destroyed blocks
  would cross. A hopper cannot eject across. A wall-facing dropper or dispenser
  keeps its stack instead of consuming it for a rejected foreign entity join.
- Normal entity movement collides with solid unknown space. Mob AI results and
  the common entity lists also have position guards for noclip or teleport-style
  movement. They stay inside before publication and at the end of a tick.
  Foreign entity joins and direct foreign summons are refused. Players awaiting
  proxy handover are held inside, including teleport requests.
- The Anvil reader is unchanged. `RegionFile` writable opens, writes and clears,
  and `RegionWriter` creates and writes, refuse foreign storage. Production
  alignment gives every writable region file one owner. The small-tile override
  can share a region file, but not a writable chunk record.

## Behaviour differences from vanilla

The wall is a hard simulation boundary. There is no entity migration, shared
redstone, fluid flow or neighbour simulation across it. Mobs and items stay on
their current side. Players wait for the proxy instead of continuing outside.
The wall can prevent an entire tree or piston move, rather than partly applying it.

Unknown owned space is also solid until it loads. This changes behaviour near
ordinary unloaded chunks, not just at a tile wall. Boundary terrain has no
decoration in its outer two chunk rings. No foreign terrain is sent to a client.
Client-only, read-only terrain is not implemented here.

This is not vanilla parity at the wall. RNG order and boundary outcomes can
differ. The position guards run after a kind's movement calculation. They do
not prove that every possible entity side effect was confined during that
calculation. The listed engine tests are the evidence, not a proof over every
entity type, portal, mod callback or command.

## Reproduction

Prepared game files are read-only at:

```text
$HOME/Library/Application Support/horizon-launcher/versions/nightly-2026-11-05-66f3981/.local/horizon-client/26.3
```

The retained E2 `horizon-ctl`, `horizon-bots` and uninstrumented baseline are
prerequisites. The baseline SHA-256 is
`88fe6c8e404625ef974be7dd44237114da485a2767d090daa64591ef2b3804ff`.
The script does not run the launcher, use its account or download game files.

```sh
./scripts/e7-tiles --build
```

`--build` can create the second checkout from the existing Horizon repository,
checks the revision, builds with `CARGO_INCREMENTAL=0`, profile `fast` and
`--no-default-features`, then retains binaries in `.local/bin/horizon-e7/`.
There is one target directory and no second build of the same workspace.
Every engine gets fresh private home and XDG directories, a fresh game folder
under `.local/runs/`, and an unused loopback port. Run folders are removed on exit.

Sections can be selected with `--section smoke|reads|writer|interior|breaches|bench|parity`.
`--mechanic NAME` retries selected breach cases and replaces their rows in the
existing result set. `--settle` controls the minimum wait. Fire, sculk, trees,
leaves and spawning wait at least five seconds.

## Evidence method

The engines are real Horizon processes. Players use real `horizon-bots` packets.
Setup and actions use console commands. `horizon-ctl` reads dedicated status.
No engine or packet path is mocked.

A control engine first seeds the fixture on both sides. Each wall direction
gets fresh masked and disabled copies of that saved world. Random-tick changes
in one direction cannot pre-run another direction's tree or leaf control.
Each attempt is bracketed by completed
`save-all` calls. The runner reads the Anvil location tables and decompresses
each stored chunk. It compares SHA-256 hashes of the compressed record payloads,
keyed by dimension, storage kind and chunk coordinates. Blocks, block entities,
ticks, POI and saved entities are included. Header timestamps are not the hash.
Oversized external records are read too. Empty chunk evidence is an error.

Each row separates an unchanged exterior from a proven breach prevention.
The latter also needs a meaningful outcome in the disabled control, absent
before the action. A failed control is not a pass. Raw rows retain commands,
replies, target observations and changed exterior record keys.

## Results

Accepted boundary result: 176 of 176 masked trials blocked, with 176 meaningful
disabled controls. No exterior record payload changed in a masked attempt.
These are 22 mechanics at east, west, north, south and all four corners, including
negative coordinates. Cardinal-only mechanisms use the x wall at a corner.

The main sweep passed 174 controls. Two single-strike lightning controls did not
deoxidize their target. The eight-strike follow-up passed all eight directions
on the same final binary and replaced the eight lightning pairs in the primary
results. `acceptance.json` summarizes `breaches.json`. The original unsuccessful
control pairs are preserved in `single-strike-controls.json`.

| Breach | Wall rule | Controls / blocked |
|---|---|---|
| setblock | Foreign target is unloaded and refused | 8 / 8 |
| fill | A straddling box is refused, not clipped | 8 / 8 |
| water | Solid unknown stops outward flow | 8 / 8 |
| lava | Solid unknown stops outward flow | 8 / 8 |
| fire | Fire and burning cannot enter foreign cells | 8 / 8 |
| sand | Falling block movement stops at the wall | 8 / 8 |
| TNT | Blast rays cannot damage foreign blocks | 8 / 8 |
| piston | A crossing push is canceled | 8 / 8 |
| hopper | Diamond stays in the source hopper | 8 / 8 |
| dropper | Diamond stays in the source dropper | 8 / 8 |
| mob | Moving pig collides and remains inside | 8 / 8 |
| arrow | Projectile stops or impacts inside | 8 / 8 |
| ender pearl | Projectile impacts the wall, no cross-tile arrival | 8 / 8 |
| boat | Unridden vehicle movement stops | 8 / 8 |
| minecart | Powered-rail vehicle movement stops | 8 / 8 |
| thrown item | Moving diamond item stays inside | 8 / 8 |
| redstone | Foreign lamp receives no update | 8 / 8 |
| lightning | Foreign oxidized copper stays unchanged | 8 / 8 |
| sculk | Catalyst-fed growth cannot enter foreign blocks | 8 / 8 |
| tree | Oak feature cannot grow a foreign trunk or canopy | 8 / 8 |
| leaves | Foreign leaf does not random-tick or decay | 8 / 8 |
| natural spawning | No saved hostile appears on the far side | 8 / 8 |

Hoppers and droppers retained their source diamond in every masked trial.
The spawning test uses a protected survival bot and only player-held loading
near the chosen wall. Fire and sculk get 400 real sprinted ticks. Spawning also
gets 400 sprinted ticks. Eight-strike lightning gets 60.

The fresh-world smoke wrote and reread an owned gold block, saved no exterior
chunks, and held an attempted player teleport to x=70 at x=63.699. Player feet
stay at least 0.301 blocks inside. Proxy arrival placement and a walking
handover with this patch were not tested.

The seven direct read positions include owned unloaded space, all four walls,
two corners and negative coordinates. `HORIZON_E7_READ_PROBE=1` reads the actual
simulation `World` before chunk loading. Disabled reads were air with light 240.
Masked reads were bedrock with light zero. All remained unloaded.
This probe does not stand in for a running lighting parity harness.

The real Anvil writer probe wrote an owned record, refused four exterior
mutations, left the owned file bytes unchanged after rejection, and created no
foreign file. Its rejection counter reached four.

## Normal play and parity

All nine interior checks matched the uninstrumented engine. The fixture compares
saved outcomes for setblock, fill, water, lava,
TNT, pistons, hoppers, droppers and redstone against the uninstrumented engine.
It is a focused behaviour check, not a tick digest or full gameplay acceptance.

This selection was invoked from the E7 checkout:

```sh
CARGO_INCREMENTAL=0 python3 -B tools/parity.py --changed HEAD --hosts '' -j 1 --timeout 60 piston_push block_update light mob_ai
```

All four harnesses stopped before a comparison. `piston_push`, `block_update`
and `mob_ai` lack `.local/minecraft/26.3/manifest.json`. `light` lacks the server
jar in that tree. The runner's automatic fresh retries failed for the same
reason. No Java oracle or Rust parity probe completed. Prepared engine resources
are not the authenticated jars and libraries those harnesses require.
No missing inputs were downloaded.

Focused Rust checks passed:

```sh
CARGO_INCREMENTAL=0 cargo test --profile fast --no-default-features -p horizon-core --lib tile_ownership
CARGO_INCREMENTAL=0 cargo test --profile fast --no-default-features -p horizon-client-render --lib tile_unknown_reads_are_solid_without_materializing_a_chunk
cargo fmt --all --check
CARGO_INCREMENTAL=0 cargo clippy --profile fast --no-default-features -p horizon-core -p horizon-world -p horizon-client-render -p horizon-client --lib --bin horizon-client --example e7_tile_write_probe -- -D warnings
git diff --check
```

The first command ran two tests. The second ran one. The patch passed a fresh
index apply check against the pinned commit plus E2, and a reverse apply check
against the working checkout.

## Timing method

Three repeats alternate uninstrumented, masked and patched-disabled engines.
Each has 50 standing packet bots and 64 pigs near spawn, view and simulation
distance 3, seed 12345, and the same flat generator. Natural spawning is disabled
before joining. The initial population and death drops are cleared after the
terrain loads, before creating the 64 pigs. Every sample must contain exactly
50 players and 64 non-player entities or the comparison fails.
Ten warmup seconds precede thirty seconds of one-second status
samples. Walls are far beyond the entire loaded footprint.

The metric is the mean of Horizon's rolling `mspt.average` windows. Samples
overlap. They are not independent per-tick measurements. This is a fast-profile
experiment, not M4 release capacity evidence. Other agents had engines running
on the same Mac, so these are not isolated equal-CPU measurements.

## Timing result

Every measured sample had 50 bots, 64 pigs and 144 resident chunks.

| Repeat | Unpatched MSPT | Masked MSPT | Patched, disabled MSPT |
|---|---:|---:|---:|
| 0 | 1.963278 | 1.459947 | 1.850578 |
| 1 | 1.470795 | 1.831222 | 1.854060 |
| 2 | 1.998505 | 1.494833 | 1.868669 |
| Mean | 1.810859 | 1.595334 | 1.857769 |

The means are -11.90% masked and +2.59% patched-disabled against unpatched
Horizon. Both pass the mean-under-5% comparison. The negative number is not a
speedup claim. One paired repeat was about 24.51% higher masked and 26.06% higher
disabled. Counts match, but tick histories and host scheduling are not isolated.
This does not establish a strict 5% ceiling. A quiet paired rerun or a CPU profile
is still needed before treating that budget as closed.

## Failures and limits

- Early runner revisions assumed the older block-state NBT schema. Java 26.3
  uses string defaults, `id` and `properties`, plus mixed-list wrappers. Those
  decoder errors were fixed before counting the full matrix.
- The dedicated endpoint rejects `horizon-ctl block` and ordinary command
  predicates reject unloaded positions before reading them. These failed queries
  are not counted as unknown-read proof.
- Initial hopper setup briefly enabled a transfer before adding power. Initial
  minecart rails formed a square network that turned carts away from the wall.
  Such controls cannot prove a wall. The corrected fixtures must pass themselves.
- A shared-world runner let a random-tick increase affect later directions'
  trees and leaves. Each direction now reopens a fresh fixture clone.
- The first apparent timing result had 88 to 112 non-player entities rather
  than a fixed 64. It is rejected, even though the masked mean was under 5%.
  `initial-bench-summary.json` retains that invalid comparison and its counts.
- Reusing the native join dispatcher initially assigned TNT and projectile IDs
  earlier without mods, which could change later mob RNG seeds inside a tile.
  The final guard preserves late ID allocation. The earlier matrix is not the
  final-binary acceptance run.
- The tree case uses a real configured oak feature. It does not prove every
  sapling growth path or tree type. The pig movement fixture does not prove
  every pathfinding goal. Portals, riding handover and all unlisted entities
  remain outside this experiment's evidence.
- No stock Java client acceptance, release benchmark or full tick digest was run.
  No full upstream Rust test suite was run.

## Ownership, disk and handoff

Only `.local/src/horizon-tiles` was changed. The E3 checkout was not edited.
The current E6 version of `horizon-01-tile-cut.patch` also touches
`crates/horizon-client-render/src/streaming.rs` and
`crates/horizon-client/src/game/levels.rs`. Those are the two file overlaps.
E2, E6 and E7 apply together in a temporary index, in that order. The combined
code was not built or run here. E7 makes no change to `dedicated.rs`,
`dedicated/`, `tile_cut`, `persist.rs`, `saving.rs` or `save.rs`.
The shared patch index and progress log receive E7 entries only.
The E2 input-trace hunk has formatting changes, not tracing behaviour changes.

The sampled shared `.local` peak was 6,413 MiB, measured every two seconds.
This includes other agents' experiments and is not an isolated E7 peak. The
final reproduction sampled its own target at 1,224 MiB. Earlier development
targets were removed by concurrent full cleanups, which caused cold rebuilds.
E7 run directories were deleted after their runs.

Final cleanup used plain `scripts/clean-builds`, following the reviewer's updated
instruction. Full cleanup is left to the orchestrator. The exact cleanup output
is in `data/E7/cleanup.json`; targets under the cap remain available.

No Folia, Paper, other GPL source or `docs/research/05-folia.md` was read.
Horizon was read at the pinned MIT revision. Its notice is in
`patches/README-E2.md`.

## Independent verification

The reviewer reran `scripts/e7-tiles` sections `breaches`, `interior`, `reads` and `writer` on 2026-11-07, writing to a separate folder. The acceptance file again reported 176 masked trials blocked, 176 positive controls and 0 changed exterior records, with a complete matrix. Results are in `data/E7/repro-*.json`. This covers the E7 binary alone. The combined binary is E10's job.
