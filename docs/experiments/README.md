# Experiment index

The reports retain failed attempts and superseded results. [Progress](../progress.md) records what the reviewer has accepted. The selected measurement stack is FINAL-09 tiles with reactor-1, not the later candidates.

| Experiment | Question | Verdict |
| --- | --- | --- |
| [E1. Durability at a tile wall](E1-durability.md) | Which commit policy survives crashes without item duplication? | P2 passes the model; real checkpoints must fit its latency budget. |
| [E2. Real transfers](E2-real-transfer.md) | What does a moving-player transfer cost on real servers? | All 100 transfers pass; walking gap P95 about 480 ms misses 150 ms. |
| [E3. Coherent cut](E3-coherent-cut.md) | Does full tile serialization fit the frozen window? | No. Normal-terrain RPC latency misses the budget. |
| [E4. Walking through walls](E4-wall-crossing.md) | Can position alone trigger reliable crossings? | 1,000 crossings pass; the early stack misses the M2 gap target. |
| [E5. Ownership store](E5-store.md) | Can a durable fenced store survive crashes? | Safety checks pass; 64-client commit latency misses the budget. |
| [E6. Fast coherent cut](E6-fast-cut.md) | Can dirty-state cuts meet the warm-cut budget? | Warm cuts and 200 restart trials pass within the supported resident footprint. |
| [E7. Tile ownership](E7-tile-ownership.md) | Can a tile refuse foreign simulation and writes? | All 176 breach checks pass with working controls; missing terrain reads solid. |
| [E8. Faster crossings](E8-fast-crossing.md) | Can prewarming and input replay remove arrival corrections? | 1,000 crossings pass without corrections; the 80 ms share is not met. |
| [E9. Group commit and P5](E9-group-commit-and-p5.md) | Does log-structured durability stay safe and cut store latency? | P5 passes the model; 64-client latency depends on the driver and remains qualified. |
| [E10. Integrated cluster](E10-cluster.md) | Do cuts, ownership, store CAS and routing work together? | M1 passes; the 1,000-crossing input-gap line passes with cut and CAS. |
| [E11. Measurement tools](E11-measurement.md) | Can bots, tick digests and equal-CPU measurements test scale? | The selected 80-tick behaviour fixture passes; early capacity rows are historical. |
| [E12. M2 and lease integration](E12-m2.md) | Can terrain, live state and local lease enforcement share one stack? | The leased stack is frozen; later E18/E23 checks supply final M2 evidence. |
| [E13. Crashes and partitions](E13-m3.md) | Does the real cluster conserve items and reject stale owners? | Final leased-engine reproduction passes 1,000 crash seeds and fencing checks; M3 closes. |
| [E14. Physics client](E14-physics-client.md) | Does a real physics client keep speed through a wall? | Early sprint failures remain; build 10 passes all 112 matched windows. |
| [E15. Store latency](E15-store-latency.md) | Can 64 clients commit below 20 ms P95 with full barriers? | Three quiet runs pass; the reviewer reproduces the store line. |
| [E16. Time-bounded authority](E16-leases.md) | Can an isolated tile stop before its authority expires? | Lease model, real-store checks and latency pass, subject to bounded critical IO. |
| [E17. Scale and behaviour](E17-m4.md) | What is the matched plain baseline, and can release measurements be prepared? | Plain-09 passes 325 twice and fails 350 twice; this is the D9 baseline. |
| [E18. Proxy cost](E18-proxy-cost.md) | Which engine and proxy costs block the capacity target? | Flat-light spin is fixed on both stacks; reactor-1 cuts proxy cost but fails at 975. |
| [E19. Capacity feasibility](E19-proxy-capacity.md) | Can tiles reach 975 if the proxy leaves the CPU cap? | No. Tile cost exceeds the stop line; diagnostic only. |
| [E20. Cluster frontier](E20-frontier.md) | What frontier passes the matched four-core budget? | Reactor-1/FINAL-09 passes 650, or 2.0x 325; combined candidates do not replace it. |
| [E21. Tile cost](E21-tile-cost.md) | What do warming and terrain readers cost? | Warm-tick reduces tile CPU; it remains a candidate, not a new frontier. |
| [E22. Shared views](E22-shared-views.md) | Can shared terrain readers save CPU without breaking delivery? | Shared-4 is held as unqualified; its saving does not pass both repeats. |
| [E23. Final-stack closeout](E23-closeout.md) | Which M2/M4 checks pass on the selected bytes? | Vehicles, physics and equal-work checks pass; latency misses, tick cost is unproven, soak fails. |
| [E24. Lease availability](E24-lease-availability.md) | Why do authority latches and memory growth interrupt the soak? | Clock-check latch fixed with a raw clock pair, renewal cadence fixed after a limiter pause, store memory bounded. `lease-fixed-r2` passes the 30-minute soak except the proxy (+16.45%) and the full M3 campaign. |
| [E27. Bots on set paths](E27-bots.md) | Do `horizon-bots` follow a set route through backend switches without reconnecting? | 40/40 routes and 435/435 switches confirmed with zero reconnects; reproduced. |

## Evidence conventions

- Store JSON evidence over 1 MB as `.json.gz`. Read it with `gzip -dc`; compression does not change the samples.
- Markdown hash tables give SHA-256 hashes of the decompressed content. For `record.json.gz`, hash the JSON bytes emitted by `gzip -dc record.json.gz`, not the gzip file.
- Harness JSON indexes hash stored file bytes. For `record.json.gz`, their digest identifies the compressed gzip bytes on disk, including the gzip container. Use `shasum -a 256 record.json.gz` for this domain. It will not equal the decompressed-content digest. For uncompressed files, the two domains coincide. Keep historical source and binary identities separate from publication hashes.
- Outside-process names become `outside` in published rows and name maps. PIDs, timing, CPU counters and registered fixture roles stay intact.
- `repro-*` names the reviewer's reproduction data. It is separate from the builder's original run, not a renamed replacement for it.
- Full native traces, screenshots and machine-specific captures stay local. Reports retain small summaries and exported counters. A local artifact path does not mean the artifact is in this repository.

## Glossary

| Term | Meaning |
| --- | --- |
| P95 / P99 | The 95th / 99th percentile. Each report states how it computes the rank and which samples it includes. |
| TPS | Ticks per second. The target is 20; capacity cells require at least 19.9 game-time TPS. |
| MSPT | Milliseconds per tick. A 20-TPS server has a 50 ms tick interval. |
| Fence | An ownership token checked on writes. A replaced or expired tile owner cannot use its old token to write. |
| CAS | Compare-and-swap. Change a record only if its expected generation and required fences still match. |
| Lease | A time-bounded authority grant. The tile must stop protected actions before its local deadline; renewal cannot revive a latched expiry. |
| Terrain reader | A held, read-only session that supplies terrain from another tile. It cannot place a player or perform gameplay writes. |
| Crossing | A player handover between tiles at a wall. Input gap measures time without accepted player action, not the whole handover duration. |
| Activation | The target makes the held player active after verifying the committed store record. Preparing a held login is not activation. |
| FINAL-09 | The selected leased engine plus flat-light patch 09 and measurement patches. Its engine hash starts `b2f1ea65`. See the [frozen identities](E23-closeout.md#frozen-identities-and-reproduction). |
| reactor-1 | The selected event-loop proxy through patch 06, before scheduler and shared-view candidates. See the [frozen identities](E23-closeout.md#frozen-identities-and-reproduction). |
| D9 | The [matched capacity layout and CPU rule](../decisions.md#d9-capacity-layout-and-equal-cpu). Both sides run the same four-square routes and 256 pigs. Servers share four aggregate cores, including cluster services; bots get one separate core. |
| Frontier rule | The highest tested player count with two valid passes and a higher step with two failures. A mixed step stays an unstable band. Players must stay online without bot lateness or dropped raw ticks; each tile needs at least 19.9 TPS and raw tick P99 below 50 ms. See [E20](E20-frontier.md#pass-rule-and-quiet-windows). |
