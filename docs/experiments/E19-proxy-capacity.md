# E19. Proxy capacity feasibility

Verdict: stop after step 1. With the proxy outside the four-core limiter, the
four tiles alone consume 3.93994 cores and still reach only 16.28 to 16.93
game-time TPS. Every tile fails the raw tick P99 limit. That exceeds the
reviewer's approximately 3.2-core tile stop line before reaching 20 TPS.
The shared terrain scheduler was not started. No patch 07, new proxy build,
commit, push or upstream change was made. The reviewer decides the next step.

This is a **diagnostic only**, not D9 or M4 acceptance. D9 includes the proxy
in the four-core server budget. This experiment deliberately excludes it.

## Step 1, proxy outside the cap

The run uses E18's frozen `reactor-1`, patch 06, and its corrected 975-player
D9 fixture. Four interior patrol cohorts hold 878 players; 97 players cross
walls and the corner. Tile edge, seeds, routes, join cadence, interior spawns,
view distance, lazy readers and the 256 pigs are unchanged. Warm-up is 30
seconds and measurement is 60 seconds. Exact process-counter endpoints span
60.1583 seconds. Native thread endpoints and game-time queries have their own
timestamps, as in E18.

Only four tiles, the store and four sidecars are in the four-core limiter,
nine PIDs. The proxy remains measured but receives no limiter signals. Five
bot drivers share their existing separate one-core cap. The five test-only
handshake shims and measurement/controller stay outside the production server
budget and are reported separately.

| Tile | Placed/held at start | Readers | Pigs | Process cores | Native main cores | Game-time TPS | Raw tick P99 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a | 247/4 | 97 | 64 | 0.99986 | 0.65051 | 16.9307 | 89.740 |
| b | 240/3 | 97 | 64 | 0.98723 | 0.64164 | 16.4129 | 84.777 |
| c | 250/5 | 97 | 64 | 0.98274 | 0.63890 | 16.2781 | 84.748 |
| d | 238/1 | 97 | 64 | 0.97011 | 0.63865 | 16.8387 | 87.091 |

Each tile retains 373 chunks. Raw tick counts are 1,031 / 1,001 / 991 / 1,026,
with zero dropped samples. Sequential status queries can straddle a handover;
the bot telemetry, rather than a sum of those queries, establishes online
population.

| Measured process set | Cores |
| --- | ---: |
| Four tiles | 3.93994 |
| Store | 0.01396 |
| Four sidecars combined | 0.00698 |
| Capped tiles, store and sidecars | 3.96088 |
| Uncapped proxy, native task counters | 1.70886 |
| Production server total, aligned process counters including proxy | 5.66971 |
| Five bot drivers, separate one-core cap | 0.63433 |
| Five test-only handshake shims, excluded | 0.95714 |
| Measurement/controller, excluded | 0.29422 |

The aligned `ps` proxy value is 1.70882 cores. The independent native task
counter gives 0.73133 user and 0.97753 system cores, with 1,991 peak threads,
944.77 MiB peak RSS, 261,047 Unix syscalls/s and 57,513 context switches/s.
Neither proxy CPU figure is from a fully passing 20-TPS workload.

| Limiter in the measurement window | Server set, proxy excluded | Bots |
| --- | ---: | ---: |
| Stop cycles | 1,673 | 193 |
| Process-stop signals | 15,057 | 965 |
| Controller stopped wall seconds | 13.3070 | 1.5534 |
| Summed per-process pause seconds | 118.1253 | 7.7164 |

All 975 players remain online in every one of the 60 sampled bot reports.
All 300 cohort rows have zero failures, late rounds and falling-behind flags.
The inherited conservative any-bot-STOP gate nevertheless fails because the
one-core bot limiter stopped the drivers, despite their average CPU headroom.
The independent tile TPS and P99 failures are sufficient to reject the cell.
No limiter pause is subtracted from any latency or tick sample.

The host-noise rule passes. Start load is 1.23584, below 4. The largest outside
process averages 0.08195 core during steady measurement and 0.08227 over the
whole cell. The quiet request lasts 201.105 seconds and is released after
cleanup, well below 30 minutes. No acknowledgement is required because the
reviewer confirmed no other agent is running. Shims and socket sampler have no
errors. The sole proxy warning is the expected pre-login TCP liveness probe
closing its connection.

## Why work stopped

The tiles already consume 0.73994 cores more than the reviewer's 3.2-core
planning line while failing both speed gates. The tile/store/sidecar limiter
still stops them for 13.3 seconds, even though the proxy is outside it.
The planned manager-thread change addresses proxy scheduling and does not
remove this measured tile-side work.

These are capped consumption figures. No precise uncapped tile demand at
20 TPS was measured, and no linear TPS-to-CPU extrapolation is used. The
result meets the reviewer's explicit feasibility stop condition; it is not a
claim that every possible engine or proxy redesign is impossible.

Steps 2 and 3 were not run. There is no new frozen candidate to declare final,
so the M2 regression campaign was not rerun. E18's earlier M2 evidence remains
unchanged, including its one unresolved physics timing window. No IO-driver
`sample` profile or 1,050-player cell was started. Two passing capped 975
runs, the exact E18 acceptance-command rerun and M4 remain unverified.

## Harness and retained evidence

`scripts/e19-production-slope` is derived from E18's runner without editing
E18. Its diagnostic switch removes only the proxy from the limiter PID list,
records capped and total server CPU separately, skips 1,050 in feasibility
mode, uses the reviewer's no-ack waiver and writes under `data/E19/`.
It also applies the requested strict P99 `< 50 ms` comparison.

`scripts/e19-tools-test` passes four tests for normal cap membership,
diagnostic exclusion, inconsistent role/PID mapping rejection, and outside
process-name redaction without losing counters. The role/PID validation was
extracted after the running cell had loaded its code. Its predicate is the
same as the executed diagnostic's exclusion. The exact executed source is
retained separately, with its hash equal to the hash captured by that run.

- Raw cell: `data/E19/feasibility-reactor-1.json.gz`, SHA-256
  `71d05bd6b573d54fa080fd2c6f77f5a70f7104afcbc8d38b1da2237974af9f28`.
- Derived summary: `data/E19/feasibility-reactor-1-summary.json`, SHA-256
  `0620e4e1612989941ff659a06a6184e51cbc5ee00bc787b1efaab21da6b4e241`.
- Exact executed source: `data/E19/feasibility-reactor-1-runner.py`, SHA-256
  `b10f28072f9ab537d4b88f894dedeaed5455efb0bb377c390eec9d220cf46efb`.
  This is a provenance copy, not a standalone entry point in that directory.
- Current tested entry point: `scripts/e19-production-slope`, SHA-256
  `7dc9af84e3736e7fbbb76cd175f24992dc7706c708251c3d0fe43527a2e00b8f`.
- Verification details: `data/E19/feasibility-reactor-1-integrity.json`.

Every published outside-process name is `outside`, both in phase rows and
the PID-to-name map. All 2,394 such entries were checked. PIDs and CPU values
are preserved. No screenshots or environment traces were created; those
would stay under `.local/evidence/`, not in published results.

All 248 Meridian files in the frozen source manifest still match. All six
retained binary hashes match `reactor-1/candidate.json`. E18's source,
patches, frozen binaries and raw data are untouched. No Rust code changed,
so no Cargo build or Rust gate was run. The Rust architecture audit was
read-only and did not change Meridian's upstream lint baseline.

The fixture's world and logs were deleted by its temporary-directory cleanup.
No E19 process, run directory or quiet marker remains. Plain
`scripts/clean-builds`, without `--all`, freed 0 MiB. The existing 38 MiB root
and 566 MiB Meridian targets remain below their caps; the project is 5,107 MiB.

## Reproduction

From the repository root, with no other quiet-window owner and a fresh name:

```sh
python3 -B scripts/e19-tools-test
python3 -B scripts/e19-production-slope --capacity-diagnostic --proxy-outside-cap --counts 975,1050 --warmup 30 --seconds 60 --bins .local/bin/meridian-e18/reactor-1 --name feasibility-reactor-repro-1
python3 -B scripts/e19-capacity-summary docs/experiments/data/E19/feasibility-reactor-repro-1.json.gz docs/experiments/data/E19/feasibility-reactor-repro-1-summary.json
scripts/clean-builds
```

The measured command used `--name feasibility-reactor-1`; other arguments
were exactly those above. The runner requires counts `975,1050` but records
1,050 as not run in proxy-outside-cap mode. Omitting that switch changes the
budget and is not a reproduction of this feasibility cell.

## Frozen proxy and hashes

No new proxy was frozen. The unchanged measured executable is
`.local/bin/meridian-e18/reactor-1/meridian`.

| Artifact in `.local/bin/meridian-e18/reactor-1/` | SHA-256 |
| --- | --- |
| meridian | `29f1ff02614019aaccd2303ea0ad300c19f632c861174ced92ec009f7903a53b` |
| meridian-sidecar | `c79c47d8ffc57081f8f0352e470f2837a089049c52bee17376f80733ff2bd65b` |
| horizon-client | `b2f1ea65ebf47658b24c55b52880ff609c658fbd9ab139d9421f9d99964bd16d` |
| horizon-bots | `bc2cc875a4d078bfa74b65432f752ec25d57758433698c98a3a40c6f8010c45d` |
| goliath-store | `6c54dbeb982470f2712555eb4059ba65ee0b110c84ec9b9266bc6d57cf6fd4af` |
| e4-client | `e9f85501d0e40ad223ae203d2f3e682c2b59d396f4eb00fbba163b817d8f0025` |
| candidate.json | `2bfd2e26c0c9ef61f3549b8a3c33faf96938875fd6063323e359c1910611d7b9` |
| canonical.json | `33a2322d27331f0b6f6310d06a8be5e4cff9adda9e98dbf1a13baf219d5c9cf9` |

## Stored raw data

Raw result files larger than 1 MB are stored gzip-compressed as `*.json.gz`. Compression is lossless, and every SHA-256 recorded in this report refers to the decompressed bytes. To rerun a summary script on one of them, decompress it first with `gzip -dk <file>.json.gz`.
