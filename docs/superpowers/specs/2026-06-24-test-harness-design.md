# Test Harness & Visual Debug Mode

## Goal

Add a CLI-driven test mode to the game that lets a developer (you or me) launch
into a deterministic scene at a fixed time of day, with a fixed camera and
spawned entities, then either capture a single screenshot for headless
inspection or stay in an interactive session with debug hotkeys.

This is a developer tool, not a player feature. It exists to:

1. Reproduce specific visual states (night, headlight pools, bloom artifacts,
   god rays) without having to walk a save over to the right place and time.
2. Capture before/after screenshots to verify visual fixes.
3. Toggle post-processing passes on/off to isolate which pass a bug lives in.
4. Free the camera to look at the scene from any angle, including fixed
   viewpoints (close to a car front, eye level with a headlight, top-down).

## Non-Goals

- No automated test runner / CI integration (screenshots stay in
  `screenshots/` for human review).
- No replay scripting.
- No changes to gameplay logic. The test mode uses the same `Game` struct
  and update/render paths as the real game.

## CLI Flags

All flags are inert unless `--test` is present. `--test` skips the title
screen and intro and starts the game in the `Playing` state.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--test` | bool | off | Enter test mode |
| `--scene=NAME` | string | `headlight_closeup` | Load a named scene preset |
| `--time=HOUR` | float | scene default | Game hour 0..24, decimals allowed (e.g. `6.5` = 6:30am) |
| `--camera=X,Y,Z` | 3 floats | scene default | World position for free camera |
| `--camera=X,Y,Z,YAW,PITCH` | 5 floats | scene default | Position + yaw (deg) + pitch (deg) |
| `--camera=behind_player` | none | scene default | Lock camera behind player vehicle |
| `--seed=N` | u64 | `0xC0FFEE` | RNG seed for city generation |
| `--cars=N` | u32 | 0 | Number of NPC traffic cars spawned |
| `--peds=N` | u32 | 0 | Number of peds spawned |
| `--screenshot=PATH` | path | none | Capture next rendered frame to PNG and exit |
| `--disable=LIST` | csv | empty | Comma-separated: `bloom,ssr,crt,godrays,fog,shadows,dyn` |
| `--freeze-time` | bool | off | Don't advance `self.time` |
| `--show-bounds` | bool | off | Draw debug AABBs (existing F3 behavior) |

## Run Modes

A single `--test` flag drives two workflows, distinguished by whether
`--screenshot=PATH` is present:

### 1. Screenshot mode (`--screenshot=out.png`)

1. Parse flags, set time, load scene, set camera.
2. Run exactly one update step (so spawned entities exist in the world
   and the post-FX passes have something to chew on).
3. Render one frame.
4. Call `r.TakeScreenshot("out.png")` (raylib's built-in, writes to working
   dir).
5. Exit cleanly with status 0.

Used for regression checks, "does my fix work" verification, and
headless-but-not-really runs.

### 2. Interactive mode (no `--screenshot`)

1. Same setup as screenshot mode.
2. Window stays open.
3. Game loop runs normally with the existing input system.
4. Additional debug hotkeys (see below) are available.

## Scene Presets

Defined in a new `src/test_scene.rs` module. Each preset is a function that,
given a `&mut Game`, sets time, position, camera, and spawns entities.
Presets are registered in a `const SCENES: &[(&str, fn(&mut Game))]` table.

Initial presets (all use `--seed=0xC0FFEE` and `--time` from preset unless
overridden on CLI):

### `headlight_closeup`

The one we need right now. Close to the front of two parked cars at night.

- `time = 22.0` (10pm)
- Player on foot at `(0, 0, 0)` looking at `+Z`
- Two cars parked side by side along `+X`, 6m apart, both pointed `+Z`:
  - `Sedan` at `(-3, 0, 4)`, `yaw=0`
  - `Sports` at `(+3, 0, 4)`, `yaw=0`
- Camera at `(0, 1.5, -3)`, `yaw=0`, `pitch=0` — looking at the fronts
  of both cars, eye level
- `cars = 0`, `peds = 0`
- Default post-processing: full

### `night_street`

Player driving on a road, traffic ahead showing headlights at night.

- `time = 21.0`
- Player in `Sedan` at origin, `yaw=0`
- 4 traffic cars spawned ahead in the same lane with `--seed` randomization
- Camera: `behind_player` (3rd person follow)
- `cars = 4`, `peds = 0`

### `dawn_drive`

Same as `night_street` but at dawn with god rays active.

- `time = 6.5`
- Camera: `behind_player`

### `parking_lot`

- `time = 19.5` (dusk)
- Player on foot
- 6 cars scattered in a rough grid
- Camera: free, default `(0, 8, 12)`, `yaw=-90` (looking down and toward `+Z`)

## Interactive Debug Hotkeys (test mode only)

| Key | Action |
|---|---|
| `F1` | Toggle existing debug overlay |
| `F3` | Toggle bounds drawing (was already there, just documented) |
| `F5` | Cycle to next scene preset |
| `F6` | Toggle free-camera vs follow-camera |
| `F7` | Toggle post-processing pass currently selected by `F8` |
| `F8` | Cycle which post-FX pass `F7` toggles |
| `Numpad +` / `Numpad -` | Adjust `time` by ±0.5 hours |
| `P` | Save screenshot to `screenshots/<timestamp>.png` |
| `L` | Cycle lighting preset: `Full` / `Day` / `Night` / `None` |
| `B` | Toggle AABB draw (alias for F3) |

These are added to the existing hotkey block in `src/main.rs`. They're
no-ops outside test mode.

## Code Changes

### `src/main.rs`

- Replace minimal arg parsing with `parse_args()` returning an
  `Args` struct: `test: bool`, `scene: String`, `time: Option<f32>`,
  `camera: CameraSpec`, `seed: u64`, `cars: u32`, `peds: u32`,
  `screenshot: Option<PathBuf>`, `disable: PostFxMask`, `freeze_time: bool`,
  `show_bounds: bool`.
- `CameraSpec` is an enum: `BehindPlayer`, `Free { pos, yaw, pitch }`.
- `PostFxMask` is a bitflags struct with bits for each pass.
- After `Game::new(...)`, if `args.test`, call `Game::enter_test_mode(args)`
  and run the appropriate mode loop.

### `src/game.rs`

- Add `Game::enter_test_mode(&mut self, args: &Args)`. Sets time, freezes
  time, applies `--disable` mask, loads scene, places player, sets
  camera, handles screenshot or interactive loop.
- Add `Game::set_time(&mut self, hour: f32)`. Also normalizes
  `total_hours` so sun direction and sky color match.
- Add `Game::capture_frame(&self) -> String` returning a path under
  `screenshots/` (creates dir).
- Extend `Game::update` to skip `self.time += dt` if `freeze_time`.

### `src/test_scene.rs` (new)

- `pub const SCENES: &[(&str, fn(&mut Game))]` table.
- Preset functions: `scene_headlight_closeup`, `scene_night_street`,
  `scene_dawn_drive`, `scene_parking_lot`.
- `pub fn apply_scene(game: &mut Game, name: &str)` — looks up by name,
  errors with a helpful list if unknown.
- Uses `cfg.test_seed`, `cfg.test_cars`, `cfg.test_peds` for randomized
  spawns.
- For deterministic NPC traffic, use the seeded RNG (already exposed in
  `crate::world::city::City`).

- Extend `FollowCamera` with a `Mode` enum: `Follow { target: Vector3, distance: f32 }` and
  `Free { pos, yaw, pitch }`.
- In test mode, `Free` mode ignores the player and uses absolute world
  coordinates. `WASD` translates the camera in the horizontal plane
  relative to its current yaw, `Q/E` moves up/down, mouse drag (with
  right-button held) yaws and pitches.

### `src/postfx.rs` (new or in `render/`)

- `pub struct PostFxMask { bloom: bool, ssr: bool, crt: bool, god_rays: bool, fog: bool, shadows: bool, dyn_sky: bool }`
- `impl PostFxMask { pub fn from_csv(s: &str) -> Self }` parses
  `--disable=bloom,crt`.

## Testing Strategy

- Unit test `parse_args` for each flag (use `clap` if it gets complex; otherwise
  hand-rolled).
- Unit test `PostFxMask::from_csv` (case insensitivity, unknown flags
  error).
- Unit test `apply_scene` rejects unknown scene names with a clear error.
- Visual: capture screenshots from each preset and review. Stored in
  `screenshots/` (gitignored).

## Risks & Open Questions

- **Free camera collision:** the free camera in `Free` mode should not
  collide with buildings or fall through the ground. For first pass,
  ignore collisions (developer tool, you can move the camera up if stuck).
- **Time wraparound:** setting `--time=25` should normalize to 1:00, not
  crash.
- **Multiple `--disable` flags:** pick the last one (or last wins, simplest).
- **raylib `TakeScreenshot` is synchronous:** blocks until the frame is
  written. Should be fine; the exit happens immediately after.
- **The headlight bug:** the user's most recent complaint is "headlights are
  still broken" plus "vertical smearing at night." Both are likely
  visible in `headlight_closeup` and `night_street` screenshots. This
  harness is the tool to reproduce, not the fix itself. Fixing the
  headlight bug and the CRT smearing is a separate follow-up.
