# City Floorlines

An open-source-style, production-grade **city-building and urban simulation** written in Rust
and [Bevy 0.20](https://bevy.org/), heavily inspired by the classic citybuilder genre.
(Not affiliated with or endorsed by the makers of *Cities: Skylines II*. 100 % original assets,
audio and text; mechanics are not copyrighted — names, logos and art are, and we ship none of theirs.)

**Architecture:** see the design doc sections delivered in-session (workspace, ECS model,
multi-rate scheduler, spline pipeline, save/modding, roadmap, multiplayer). This README is the
operational index.

## Crate map (Section 1 + 7 of the design doc)

| Crate | Role | Engine deps |
|---|---|---|
| `city_core` | ids, tick math, enums, OBB/AABB, spatial grid hash, chunked dense grids, player intents | none |
| `city_networks` | cubic Bézier w/ arc-length LUTs, CSR directed lane graph, congestion-aware A* (CCH planned) | none |
| `city_terrain` | heightfield, procedural generation, road cut-and-fill blending | `noise`, `rayon` |
| `city_io` | `.cflsave` container (magic `CFLR`, crc32 chunks, zstd), RON building descriptors | `serde`, `bincode`, `zstd`, `ron` |
| `city_sim` | deterministic 60 Hz `FixedUpdate` lane, tick-divisor buckets (4 Hz / 1 Hz / 0.2 Hz), intent pipeline (single-player & multiplayer share it), municipal budget | `bevy_app/ecs` |
| `city_render` | quality tiers (Potato/Standard/High/Ultra), exposure/EV100 state | minimal |
| `city_ui` | HUD state, heatmap modes | minimal |
| `city_net` | replication whitelist, interest tiles (128 m), divergence hash, cadence policy (transport: Phase 7) | `serde` |
| `city_app` | game binary (`cityfloorlines`) — terrain, buildings, sun, orbit camera | full bevy |
| `city_server` | headless `cityfloorlines-server` — replays, determinism CI, dedicated sim | `bevy_app/ecs/time` |

## Status vs the roadmap (Phase 1 vertical slice)

**Implemented & tested:** workspace skeleton; deterministic tick lane with bucket conditions;
intent queue + validation (insufficient-funds rejection path); spatial grid (O(1) agents);
CSR graph with congestion-aware A*; cubic Bézier meshing substrate; heightfield terrain with
deterministic procedural generation; cut-and-fill solver; save/load chunk container with crc32 +
zstd + schema versioning; RON descriptor parser (mod pipeline); multiplayer replication contract
(whitelist, interest tiles, divergence hash); headless server binary printing world hash.

**Not yet implemented (in the plan, deliberately not stubbed here):**
meshlet/Hi-Z pipeline, Solari GI & DLSS, weather materials, OIT glass, intersection geometry
generation, Voronoi/OBB parceling, CCH pathfinding (A* interim), economy IO-matrix solve,
public transit sim, districts/policies/milestones, Rhai/Rust mod execution, `lightyear`/`bevy_replicon`
transport (Phase 7), BSN building scenes (the `.bsn` *asset format* ships after Bevy 0.20).

## Build & run

```sh
cargo test --workspace            # 43 unit tests, engine-free crates run anywhere
cargo run -p city_app             # opens the game window (procedural terrain + buildings)
cargo run -p city_server --       # headless sim, prints day-by-day log + world hash
CFL_EXIT_FRAMES=120 cargo run -p city_app   # CI smoke run: exits after 120 frames
```

Camera (Phase 1 slice): **RMB/MMB drag** orbit, **wheel** zoom, **WASD** pan.

## Layout

```
Cargo.toml            workspace
crates/city_*         one crate per concern — see table above
```
