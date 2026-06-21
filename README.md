# GTA7

A GTA-style 3D game built in Rust with [Raylib](https://www.raylib.rs). Full 3D city, driving, on-foot combat, AI, wanted system, missions — all procedurally generated, zero external assets.

## Features

- **Procedural city** — seeded grid of blocks with buildings, roads, lane markings, parks, trees, traffic lights
- **Driving** — arcade vehicle physics: throttle, steering, handbrake, crash damage, explosions
- **On-foot** — third-person movement, sprint, jump, mouse-look orbit camera
- **Combat** — pistol + SMG hitscan with tracers, muzzle flash, blood, reload, melee
- **AI** — pedestrians wander/flee, traffic follows lane graph + stops at red lights, cops chase and shoot
- **Wanted system** — 6-star escalating police response, heat decay when out of sight
- **Health/armor** — damage, fall/crash damage, death + respawn at hospital with cash penalty
- **Money + pickups** — health, armor, cash, weapon pickups; weapon + health shops
- **Missions** — rotating objectives (reach point, kill target, deliver car, survive) with cash rewards
- **HUD** — health/armor bars, money, wanted stars, weapon/ammo, minimap, mission banner, crosshair
- **Fixed timestep** — 30/60/90/120 Hz selectable logic rate with render interpolation for fluid motion

## Controls

| Key | Action |
|-----|--------|
| `WASD` | Move / drive |
| `Mouse` | Look around |
| `Left Click` | Fire weapon |
| `Space` | Jump (on foot) / Handbrake (driving) |
| `Shift` | Sprint |
| `F` | Enter/exit vehicle / Melee (unarmed) |
| `R` | Reload |
| `E` | Interact (shop) |
| `Tab` / `Q` | Switch weapon |
| `V` | Melee attack |
| `F1` | Toggle debug overlay |
| `F2` | Cycle logic rate (30→60→90→120 Hz) |

## Build & Run

```bash
cargo run          # debug
cargo run --release  # optimized
```

### System dependencies

Raylib builds its own C source via `raylib-sys`. You need a C compiler and OpenGL development headers:

- **Linux:** `gcc`, `libgl1-mesa-dev`, `libx11-dev`, `libxcursor-dev`, `libxrandr-dev`, `libxi-dev`, `libxinerama-dev`
- **Windows:** MSVC build tools (Visual Studio C++ build tools)
- **macOS:** Xcode command line tools

```bash
# Arch / CachyOS
sudo pacman -S base-devel mesa libx11 libxcursor libxrandr libxi libxinerama

# Debian / Ubuntu
sudo apt install build-essential libgl1-mesa-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxinerama-dev
```

## Architecture

```
src/
  main.rs            — window, main loop, fixed-timestep + interpolation
  lib.rs             — module root
  config.rs          — settings, logic rate, palette, procedural colors
  time.rs            — accumulator clock with alpha interpolation
  input.rs           — input sampling + edge detection
  mathx.rs           — vector math, lerp, angle wrap, helpers
  game.rs            — Game state, update orchestration, spawn/wanted/mission logic
  hud.rs             — HUD: bars, money, stars, minimap, banners
  player.rs          — on-foot controller, health, weapons, inventory
  vehicle.rs         — arcade car physics + damage + explosion
  camera.rs          — orbit (on foot) / chase (driving) follow camera
  combat.rs          — hitscan shooting, melee, cop fire, damage routing
  wanted.rs          — heat accumulation, star levels, decay
  pickup.rs          — pickups + shops
  mission.rs         — rotating objective system
  world/
    city.rs          — procedural grid city, lane graph, traffic lights
    collision.rs     — circle/AABB + ray/AABB
  ai/
    ped.rs           — pedestrian state machine (wander/flee/dead)
    cop.rs           — cop AI (chase/shoot/dead)
    traffic.rs       — traffic lane-following + red light stops
  render/
    models.rs        — procedural textures, textured models, draw helpers
    fx.rs            — particles, tracers, muzzle flash, explosions
```

## Engine: Fixed Timestep + Interpolation

Logic runs at a selectable fixed rate (30/60/90/120 Hz, default 60, cycle with `F2`). Rendering is decoupled at vsync. An accumulator tracks leftover time and computes an interpolation alpha (`accumulated / dt`), so entity transforms are lerped between the previous and current logic states. This keeps 30Hz logic visually fluid and 120Hz crisp — without full variable-rate integration.

## Tests

```bash
cargo test
```

Unit tests cover: collision resolution (circle/AABB push-out, ray/AABB hit + normal), wanted heat math (add/decay/star thresholds), vehicle physics (acceleration, steering, damage), procgen determinism (same seed → same city).

## Tech

- **Rust 1.95** + **raylib 6.0** (safe bindings)
- **rand + rand_chacha** for deterministic seeded procgen
- No external assets — all textures and models generated in code
- Single crate, fast compile
