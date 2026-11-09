# E26. Mechanical hunk coverage

Generated from every diff hunk in the selected stack, plus the unselected patches 10 and 11.
Each hunk has one row. Runtime acceptance and timer overhead remain separate gates.

| Hunk | Source | Timer or exclusion |
| --- | --- | --- |
| `e2-horizon-input-trace.patch:5` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `e2-horizon-input-trace.patch:14` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `e2-horizon-input-trace.patch:23` | `crates/horizon-client/src/lan/hub.rs` | Signature only. The trace predicate and body have an inline timer. |
| `e2-horizon-input-trace.patch:31` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-01-tile-cut.patch:5` | `crates/horizon-client-render/src/block_entities.rs` | declaration only; no runtime instruction |
| `horizon-01-tile-cut.patch:15` | `crates/horizon-client-render/src/block_entities.rs` | declaration only; no runtime instruction; test-only |
| `horizon-01-tile-cut.patch:47` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:85` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:93` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:101` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:119` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:127` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:135` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:143` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:151` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:159` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:169` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:177` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:185` | `crates/horizon-client-render/src/block_entities.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:197` | `crates/horizon-client-render/src/streaming.rs` | declaration only; no runtime instruction |
| `horizon-01-tile-cut.patch:206` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state; includes a labelled local upper bound |
| `horizon-01-tile-cut.patch:214` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:222` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:230` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:238` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:246` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:267` | `crates/horizon-client/src/command_storage.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:280` | `crates/horizon-client/src/game/bossbars.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:294` | `crates/horizon-client/src/game/dedicated.rs` | declaration only; no runtime instruction |
| `horizon-01-tile-cut.patch:302` | `crates/horizon-client/src/game/dedicated.rs` | declaration only; no runtime instruction |
| `horizon-01-tile-cut.patch:311` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:319` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-01-tile-cut.patch:327` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:336` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:349` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-01-tile-cut.patch:383` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-01-tile-cut.patch:400` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:1168` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:1935` | `crates/horizon-client/src/game/dragon.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:1948` | `crates/horizon-client/src/game/functions.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:1961` | `crates/horizon-client/src/game/levels.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:1979` | `crates/horizon-client/src/game/levels.rs` | Direct pacing-loop timer 4, nested warm timer 3 and upstream pump timer 8. Sleep is paused explicitly. |
| `horizon-01-tile-cut.patch:2002` | `crates/horizon-client/src/game/maps.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2015` | `crates/horizon-client/src/game/nether_portal.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2028` | `crates/horizon-client/src/game/persist.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2043` | `crates/horizon-client/src/game/persist.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2104` | `crates/horizon-client/src/game/raids.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2119` | `crates/horizon-client/src/game/scoreboard.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2129` | `crates/horizon-client/src/game/scoreboard.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2142` | `crates/horizon-client/src/game/trading.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2157` | `crates/horizon-client/src/game/wandering_trader.rs` | 4 control/transfer |
| `horizon-01-tile-cut.patch:2172` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:2212` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:2242` | `crates/horizon-client/src/save.rs` | Visibility only. Calls during cut encoding remain inside cut timer 2. |
| `horizon-01-tile-cut.patch:2251` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:2260` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-01-tile-cut.patch:2282` | `crates/horizon-client/src/stopwatches.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:5` | `crates/horizon-client-render/src/chunk_map.rs` | declaration only; no runtime instruction |
| `horizon-02-tile-ownership.patch:13` | `crates/horizon-client-render/src/chunk_map.rs` | 0 ownership; includes a labelled local upper bound |
| `horizon-02-tile-ownership.patch:21` | `crates/horizon-client-render/src/chunk_map.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:31` | `crates/horizon-client-render/src/chunk_map.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:41` | `crates/horizon-client-render/src/chunk_map.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:61` | `crates/horizon-client-render/src/collisions.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:71` | `crates/horizon-client-render/src/collisions.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:112` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:122` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:132` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:142` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:152` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:165` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:175` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-02-tile-ownership.patch:189` | `crates/horizon-client-render/src/world.rs` | declaration only; no runtime instruction |
| `horizon-02-tile-ownership.patch:199` | `crates/horizon-client-render/src/world.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:216` | `crates/horizon-client-render/src/world.rs` | 0 ownership; includes a labelled local upper bound |
| `horizon-02-tile-ownership.patch:225` | `crates/horizon-client-render/src/world.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:235` | `crates/horizon-client-render/src/world.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:246` | `crates/horizon-client-render/src/world.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:255` | `crates/horizon-client-render/src/world.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:269` | `crates/horizon-client-render/src/world.rs` | declaration only; no runtime instruction; test-only |
| `horizon-02-tile-ownership.patch:296` | `crates/horizon-client/src/game.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:314` | `crates/horizon-client/src/game.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:325` | `crates/horizon-client/src/game.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:333` | `crates/horizon-client/src/game.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:345` | `crates/horizon-client/src/game/commands.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:360` | `crates/horizon-client/src/game/entity.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:400` | `crates/horizon-client/src/game/levels.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:420` | `crates/horizon-client/src/game/machines.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:430` | `crates/horizon-client/src/game/machines.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:446` | `crates/horizon-client/src/game/native_entities.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:490` | `crates/horizon-client/src/game/native_entities.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:503` | `crates/horizon-client/src/game/postprocess.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:522` | `crates/horizon-client/src/game/world_loading.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:567` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-02-tile-ownership.patch:582` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-02-tile-ownership.patch:596` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-02-tile-ownership.patch:624` | `crates/horizon-client/src/mobs.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:658` | `crates/horizon-client/src/ticket_levels.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:672` | `crates/horizon-core/src/lib.rs` | declaration only; no runtime instruction |
| `horizon-02-tile-ownership.patch:685` | `crates/horizon-core/src/tile_ownership.rs` | 0 ownership |
| `horizon-02-tile-ownership.patch:829` | `crates/horizon-world/examples/e7_tile_write_probe.rs` | off-thread fixture driver |
| `horizon-02-tile-ownership.patch:883` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:909` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:917` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:925` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:933` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-02-tile-ownership.patch:941` | `crates/horizon-world/src/region.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:4` | `Cargo.lock` | declaration only |
| `horizon-03-cluster.patch:22` | `Cargo.lock` | declaration only |
| `horizon-03-cluster.patch:33` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-03-cluster.patch:48` | `crates/horizon-client-render/src/streaming.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:57` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state; includes a labelled local upper bound |
| `horizon-03-cluster.patch:66` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-03-cluster.patch:78` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-03-cluster.patch:86` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-03-cluster.patch:96` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-03-cluster.patch:114` | `crates/horizon-client/Cargo.toml` | declaration only |
| `horizon-03-cluster.patch:125` | `crates/horizon-client/src/benchmark.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:136` | `crates/horizon-client/src/control.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:144` | `crates/horizon-client/src/control.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:163` | `crates/horizon-client/src/game/dedicated.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:171` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:181` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:199` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:443` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-03-cluster.patch:463` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-03-cluster.patch:477` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:485` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | Removed fields. No surviving runtime instruction. |
| `horizon-03-cluster.patch:495` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | Field declaration. The final Arc representation replaces this field in patch 5; construction is inside cut timer 2. |
| `horizon-03-cluster.patch:503` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:512` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | Removed initializers. No surviving runtime instruction. |
| `horizon-03-cluster.patch:522` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:544` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:553` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:561` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:600` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | Removed validation. No surviving runtime instruction. |
| `horizon-03-cluster.patch:627` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:683` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:716` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:728` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-03-cluster.patch:740` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 5 observer control; includes a labelled local upper bound |
| `horizon-03-cluster.patch:787` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-03-cluster.patch:798` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:812` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:829` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:843` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-03-cluster.patch:861` | `crates/horizon-client/src/game/lan.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:883` | `crates/horizon-client/src/game/persist.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:893` | `crates/horizon-client/src/game/persist.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:914` | `crates/horizon-client/src/game/ui_control.rs` | 4 control/transfer |
| `horizon-03-cluster.patch:925` | `crates/horizon-client/src/lan/hub.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:933` | `crates/horizon-client/src/lan/hub.rs` | declaration only; no runtime instruction |
| `horizon-03-cluster.patch:941` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-03-cluster.patch:949` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-03-cluster.patch:964` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-03-cluster.patch:976` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-03-cluster.patch:988` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-03-cluster.patch:1008` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-05-m2.patch:4` | `crates/horizon-client-render/src/streaming.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:13` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-05-m2.patch:22` | `crates/horizon-client-render/src/streaming.rs` | 2 cut state |
| `horizon-05-m2.patch:44` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-05-m2.patch:58` | `crates/horizon-client/src/game/dedicated/status.rs` | Call replaced by the authority-guarded call in patch 6. The full warm callee has exclusive timer 3. |
| `horizon-05-m2.patch:69` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:77` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:91` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:100` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:164` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:184` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:192` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:228` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:239` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:269` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:300` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 3 warming |
| `horizon-05-m2.patch:396` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:406` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:434` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-05-m2.patch:446` | `crates/horizon-client/src/game/dedicated/transfer.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:454` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 5 observer control; includes a labelled local upper bound |
| `horizon-05-m2.patch:533` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer; 5 observer control |
| `horizon-05-m2.patch:578` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:591` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:601` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:609` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:619` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:633` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-05-m2.patch:669` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-05-m2.patch:702` | `crates/horizon-client/src/game/dedicated/transfer/live.rs` | 4 control/transfer |
| `horizon-05-m2.patch:867` | `crates/horizon-client/src/game/lan.rs` | Visibility only. No new runtime instruction. |
| `horizon-05-m2.patch:876` | `crates/horizon-client/src/game/lan.rs` | Visibility only. No new runtime instruction. |
| `horizon-05-m2.patch:885` | `crates/horizon-client/src/game/lan.rs` | 4 control/transfer |
| `horizon-05-m2.patch:893` | `crates/horizon-client/src/game/lan.rs` | 4 control/transfer |
| `horizon-05-m2.patch:941` | `crates/horizon-client/src/lan/arrivals.rs` | 4 control/transfer |
| `horizon-05-m2.patch:956` | `crates/horizon-client/src/lan/arrivals.rs` | 4 control/transfer |
| `horizon-05-m2.patch:968` | `crates/horizon-client/src/lan/hub.rs` | declaration only; no runtime instruction |
| `horizon-05-m2.patch:977` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:985` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:999` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:1009` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:1017` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:1027` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:1070` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-05-m2.patch:1084` | `crates/horizon-client/src/lan/import.rs` | 4 control/transfer |
| `horizon-05-m2.patch:1093` | `crates/horizon-client/src/lan/import.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-05-m2.patch:1114` | `crates/horizon-client/src/lan/import.rs` | 4 control/transfer |
| `horizon-05-m2.patch:1163` | `crates/horizon-items/src/cooldowns.rs` | 4 control/transfer |
| `horizon-05-m2.patch:1198` | `crates/horizon-items/src/cooldowns.rs` | declaration only; no runtime instruction; test-only |
| `horizon-06-leases.patch:4` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-06-leases.patch:12` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-06-leases.patch:52` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer; includes a labelled local upper bound |
| `horizon-06-leases.patch:71` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-06-leases.patch:79` | `crates/horizon-client/src/game/dedicated.rs` | 4 control/transfer |
| `horizon-06-leases.patch:90` | `crates/horizon-client/src/game/dedicated/cluster.rs` | declaration only; no runtime instruction |
| `horizon-06-leases.patch:105` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-06-leases.patch:124` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-06-leases.patch:146` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-06-leases.patch:154` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-06-leases.patch:162` | `crates/horizon-client/src/game/dedicated/cluster.rs` | 4 control/transfer |
| `horizon-06-leases.patch:175` | `crates/horizon-client/src/game/dedicated/cluster/lease.rs` | 1 lease |
| `horizon-06-leases.patch:520` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status |
| `horizon-06-leases.patch:539` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-06-leases.patch:549` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status |
| `horizon-06-leases.patch:563` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-06-leases.patch:588` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-06-leases.patch:605` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | 2 cut state |
| `horizon-06-leases.patch:616` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:640` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:670` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:691` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:701` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:715` | `crates/horizon-client/src/game/dedicated/tile_cut/store.rs` | 2 cut state |
| `horizon-06-leases.patch:726` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-06-leases.patch:736` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-06-leases.patch:744` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-06-leases.patch:752` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 4 control/transfer |
| `horizon-06-leases.patch:764` | `crates/horizon-client/src/game/dedicated/transfer/live.rs` | 4 control/transfer |
| `horizon-06-leases.patch:782` | `crates/horizon-client/src/game/lan.rs` | 4 control/transfer |
| `horizon-06-leases.patch:795` | `crates/horizon-client/src/game/lan_actions.rs` | 4 control/transfer |
| `horizon-06-leases.patch:808` | `crates/horizon-client/src/lan/conn.rs` | 4 control/transfer |
| `horizon-06-leases.patch:818` | `crates/horizon-client/src/lan/conn.rs` | 4 control/transfer |
| `horizon-06-leases.patch:828` | `crates/horizon-client/src/lan/conn.rs` | 4 control/transfer |
| `horizon-06-leases.patch:841` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-06-leases.patch:855` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-06-leases.patch:869` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-06-leases.patch:887` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-06-leases.patch:903` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-06-leases.patch:922` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-06-leases.patch:959` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-06-leases.patch:967` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-06-leases.patch:975` | `crates/horizon-client/src/save.rs` | 2 cut state |
| `horizon-06-leases.patch:987` | `crates/horizon-core/src/authority.rs` | 1 lease |
| `horizon-06-leases.patch:1013` | `crates/horizon-core/src/lib.rs` | declaration only; no runtime instruction |
| `horizon-06-leases.patch:1024` | `crates/horizon-core/src/tile_ownership.rs` | 0 ownership |
| `horizon-07-observer-status.patch:4` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-07-observer-status.patch:40` | `crates/horizon-client/src/game/dedicated/transfer.rs` | declaration only; no runtime instruction |
| `horizon-07-observer-status.patch:56` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 5 observer control; includes a labelled local upper bound |
| `horizon-07-observer-status.patch:74` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 5 observer control; includes a labelled local upper bound |
| `horizon-07-observer-status.patch:90` | `crates/horizon-client/src/game/dedicated/transfer.rs` | 5 observer control; includes a labelled local upper bound |
| `horizon-07-observer-status.patch:105` | `crates/horizon-client/src/game/lan.rs` | 4 control/transfer; 5 observer control; includes a labelled local upper bound |
| `horizon-07-observer-status.patch:131` | `crates/horizon-client/src/lan/hub.rs` | declaration only; no runtime instruction |
| `horizon-07-observer-status.patch:140` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:148` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:158` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:168` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:178` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:187` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:200` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:209` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-07-observer-status.patch:217` | `crates/horizon-client/src/lan/hub.rs` | 5 observer control |
| `horizon-09-flat-light.patch:4` | `crates/horizon-client-lib/Cargo.toml` | declaration only |
| `horizon-09-flat-light.patch:18` | `crates/horizon-client/src/flatgen.rs` | 4 control/transfer |
| `horizon-09-flat-light.patch:28` | `crates/horizon-client/src/flatgen.rs` | declaration only; no runtime instruction; test-only |
| `horizon-09-flat-light.patch:64` | `crates/horizon-client/tests/flat_light.rs` | test-only |
| `horizon-03-bots.patch:5` | `Cargo.lock` | declaration only |
| `horizon-03-bots.patch:17` | `crates/horizon-bots/Cargo.toml` | off-thread fixture driver |
| `horizon-03-bots.patch:29` | `crates/horizon-bots/src/main.rs` | off-thread fixture driver |
| `horizon-03-bots.patch:38` | `crates/horizon-bots/src/main.rs` | off-thread fixture driver |
| `horizon-03-bots.patch:57` | `crates/horizon-bots/src/measured.rs` | off-thread fixture driver |
| `horizon-04-digest.patch:5` | `crates/horizon-client/src/game.rs` | declaration only; no runtime instruction |
| `horizon-04-digest.patch:17` | `crates/horizon-client/src/game/dedicated/status.rs` | excluded adapted handler; horizon-08 supplies the final handler |
| `horizon-04-digest.patch:26` | `crates/horizon-client/src/game/dedicated/status.rs` | excluded adapted handler; horizon-08 supplies the final handler |
| `horizon-04-digest.patch:39` | `crates/horizon-client/src/game/e11_measure.rs` | 7 digest/samples |
| `horizon-04-digest.patch:296` | `crates/horizon-client/src/game/levels.rs` | 4 control/transfer |
| `horizon-04-digest.patch:304` | `crates/horizon-client/src/game/levels.rs` | 4 control/transfer |
| `horizon-04-digest.patch:314` | `crates/horizon-client/src/game/levels.rs` | 4 control/transfer |
| `horizon-08-m4-measurement.patch:4` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-08-m4-measurement.patch:13` | `crates/horizon-client/src/game/dedicated/status.rs` | 6 status; includes a labelled local upper bound |
| `horizon-10-warm-tick.patch:4` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-10-warm-tick.patch:13` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-10-warm-tick.patch:21` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-10-warm-tick.patch:33` | `crates/horizon-client/src/game/dedicated/tile_cut.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:4` | `crates/horizon-client/src/game/dedicated/status.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:19` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:40` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:64` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:92` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:108` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:132` | `crates/horizon-client/src/game/dedicated/transfer.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:152` | `crates/horizon-client/src/game/lan.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:165` | `crates/horizon-client/src/game/lan.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:185` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:194` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:203` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:211` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:244` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:253` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:261` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:272` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:282` | `crates/horizon-client/src/lan/hub.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:294` | `crates/horizon-client/src/lan/hub/tests/observer_tests.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:569` | `crates/horizon-client/src/lan/mod.rs` | not in FINAL-09; candidate patch 10 or 11 |
| `horizon-11-shared-observer.patch:581` | `crates/horizon-client/src/lan/observer.rs` | not in FINAL-09; candidate patch 10 or 11 |
