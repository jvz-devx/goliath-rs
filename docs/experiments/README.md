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
| [E25. Proxy memory and latency](E25-proxy.md) | Can the proxy pass the 30-minute memory line and add under 5 ms P95? | Rebuilt reactor-1 `80f` passes memory (+1.95%, reproduced +2.59%); the frozen build of the same source fails twice, cause open. Latency fails under D9 (11.16 and 5.72 ms), with the tail tied to limiter pauses. |
| [E26. Tick profile and tile cost](E26-tick-profile.md) | Does anything we added cost over 5% of the busiest tile's tick, and can the tile get cheaper? | Speculative warming alone took 5.24% of main-thread CPU (failed, reproduced). Patch 10 cuts it to 0.38%; the combined additions stay unproven. |
| [E27. Bots on set paths](E27-bots.md) | Do `horizon-bots` follow a set route through backend switches without reconnecting? | 40/40 routes and 435/435 switches confirmed with zero reconnects; reproduced. |
| [E28. Second platform](E28-linux.md) | Do the M4 results hold on a 6-core Linux machine, with the CPU budget enforced by core pinning instead of the limiter? | Baseline 200 and frontier 425 reproduce, 2.125x against the Mac's 2.0x. Pinned proxy P95 is under 5 ms; the limiter comparison is inconclusive. Warming costs about 0.6%, combined additions unproven. The soak fails on this machine: two bugs are fixed and a store-stall cause is still open. M3 is in progress. |

## Evidence conventions

- Store JSON evidence over 1 MB as `.json.gz`. Read it with `gzip -dc`; compression does not change the samples.
- Markdown hash tables give SHA-256 hashes of the decompressed content. For `record.json.gz`, hash the JSON bytes emitted by `gzip -dc record.json.gz`, not the gzip file.
- Harness JSON indexes through E27 hash stored file bytes. For `record.json.gz`, their digest identifies the compressed gzip bytes on disk, including the gzip container. Use `shasum -a 256 record.json.gz` for this domain. It will not equal the decompressed-content digest. For uncompressed files, the two domains coincide. Keep historical source and binary identities separate from publication hashes.
- E28's harness indexes name their domain. A `*.hash.json` or `*-hash.json` file beside a row, and the `sha256` or `decompressed_sha256` field of an E28 ledger or evidence index, hash the decompressed bytes. `stored_sha256` hashes the gzip file. On Linux, `sha256sum` replaces `shasum -a 256`.
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
| D12 | The [second-platform rule](../decisions.md#d12-second-platform-linux-with-core-pinning). On the 6-core Linux machine D9's budget is enforced by pinning: CPUs 0 to 3 run the servers, CPU 4 the bots, CPU 5 the harness and the rest of the host. D9's validity rules still apply. A cell is also invalid when a thermal-throttle counter rises or CPU 5 is 95% busy or more. Each platform's frontier is compared only with its own baseline. |
| Pin and limiter | The two ways to enforce the CPU budget, chosen with `--cpu-mode`. `pin` places each group in a cgroup v2 cpuset and never pauses a process. `limiter` is the Mac's method: a controller sends SIGSTOP and SIGCONT to hold a group to its average core count. On Linux the limiter is a comparison mode, with servers and bots sharing CPUs 0 to 4. |
| CPU 5 busy | The share of time CPU 5 was not idle, from cell start to the end of the measured window. It covers the harness and every outside process, so it shows whether the measuring side had room. |
| Outside process | Any process that is not part of the fixture. The reports give the largest one and the total, in cores. A cell is invalid when one outside process averages more than 0.5 core. |
| Bundle | A frozen folder of release binaries and build records under `.local/bin/goliath-e28/`, hashed file by file in E28's [build manifest](data/E28/build-manifest.json). A runner checks the bundle before and after each cell. E28 has `plain-09` (the baseline server), `final` (the final stack), `final-lite` (final with the warming timer), `final-stamps` (final with latency timestamps, and the source of the direct bots) and `final-r2` (final with the two soak fixes in the tile). |
| Final stack | FINAL-09 tiles plus the lease-renewal patch `horizon-13` r2 and the warm-tick patch 10, the bounded store, and the rebuilt reactor-1 proxy. It is the stack E28 rebuilds and measures. |
| Gate disposition | The reviewer's [record](data/E28/gate-disposition.json) of which failed build gates are accepted, by name and by bundle. E28 accepts four formatting and lint failures caused by the newer toolchain. Any other failed gate blocks a bundle. |
| Front shim | A test-only relay between the bots and the proxy that rewrites the host name in the first packet, so each bot group logs in on its own tile. It is not part of the server budget. On the Linux machine it saturated CPU 5 and held back bot traffic. |
| Direct bots | Bots that put their tile's host name in the first packet themselves, so no shim is needed. E28 uses the `final-stamps` bot binary for this with its timestamp tracing switched off. |
| Harness v1, v2, v3 | The three ways E28's cluster cells routed bots: v1 through the original decoding shim, v2 through a shim that only forwards bytes, v3 with direct bots. Only v3 rows count toward the Linux frontier. |
| Lite row | A tick-cost cell from E26b. An A row runs the untimed stack as a control. A B row runs the `lite` build, which times speculative warming with the thread CPU clock. A B row passes the intrusion check when its main-thread CPU and raw tick P50 and P99 stay inside the range of the four A rows. |
| Barrier | The store's durable flush of one batch of commands to its journal. A slow barrier delays every reply in that batch, lease renewals included. |
| `-a1` attempt | A retry registered by name before it runs, allowed only after a collection or harness failure. The failed row stays in the data, and performance failures are never retried. |
| Whole-window evidence | D12 counters taken once a second over a whole host window, from acquisition through recovery and cleanup, instead of over one cell. The soak and M3 use it. |
