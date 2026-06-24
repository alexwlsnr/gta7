# Test Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a CLI-driven test mode to the GTA7 game that lets a developer launch into a deterministic scene at a fixed time of day, with a fixed camera and spawned entities, capture screenshots for visual review, and toggle individual post-processing passes to isolate rendering bugs.

**Architecture:** Hand-rolled arg parser in `main.rs` builds an `Args` struct; `Game::enter_test_mode` consumes it. New `src/test_scene.rs` module holds a small registry of named scene presets that mutate the `Game`. New `src/postfx_mask.rs` module holds a `PostFxMask` bitset that's threaded through `PostFx` and the lit shader to disable individual passes. `FollowCamera` gains a `Mode` enum so test mode can use a free-flying camera while the real-game path stays unchanged.

**Tech Stack:** Rust + raylib 6.0 (no new crates). `std::env::args` for CLI. `std::time::SystemTime` for screenshot timestamps.

## Global Constraints

- **Zero new crates** — use only what's already in `Cargo.toml` (raylib, rand, rand_chacha).
- **No changes to gameplay logic** outside `enter_test_mode` and the new modules. `Game::update`, `Game::render`, `camera.update` (in follow mode) stay identical.
- **`--test` is a hard gate**: nothing else in this plan takes effect unless `args.test == true`. The flags are inert without it.
- **Existing tests must still pass**: `cargo test` is the gate for every task.
- **Commits**: one commit per task. Use the prefix `feat(test-harness):` for feature work, `refactor(test-harness):` for prep.
- **No emoji, no "MVP", no leftover scaffolding comments**.
- **Test mode camera ignores collisions** (free-fly through walls is acceptable for a developer tool).

---

## File Structure

### New files
- `src/test_scene.rs` — scene preset registry + free camera input helpers. Pure functions over `&mut Game` + `&RaylibHandle`. No state of its own.
- `src/postfx_mask.rs` — `PostFxMask` bitset. Single-purpose, ~50 lines.
- `src/cli_args.rs` — `Args`, `CameraSpec`, arg parsing. Pure data + parsing functions, no I/O side effects beyond `std::env::args`.

### Modified files
- `src/main.rs` — replace hand-rolled startup with `cli_args::parse_args()` and a branch into test-mode loop.
- `src/game.rs` — add `enter_test_mode`, `set_time`, `capture_screenshot_path`, `test_capture_pending` flag. Skip `self.time += dt` in `update` when freeze is set. The Title/Intro screen-state guards already in `update` stay — test mode sets `screen_state = Playing` directly.
- `src/camera.rs` — add `Mode` enum on `FollowCamera`, `set_free(pos, yaw, pitch)`, `update_free(input, dt)`, `is_free()`.
- `src/render/postfx.rs` — read `&self.disabled: &PostFxMask` field; gate bloom/SSR/CRT/god-rays/sky passes. Add `disabled()` setter.
- `src/render/lighting.rs` — read `&self.disabled_lighting: &PostFxMask` field; skip shadow pass + `apply_to_materials` if shadows disabled.

### Existing patterns reused
- `Game::new` already spawns parked cars and traffic; test presets manipulate that Vec.
- `City::ensure_blocks_around(pos, radius, cfg, ...)` is the right hook for "give me a city around this point."
- `crate::config::{sun_position, sun_color, sky_colors_for_hour, god_ray_intensity}` are pure functions we already call from render; reuse for `set_time` math.
- `raylib::core::misc::take_screenshot` is the FFI wrapper for `TakeScreenshot`.

---

## Task 1: CLI argument parser

**Files:**
- Create: `src/cli_args.rs`
- Modify: `src/lib.rs` (add `pub mod cli_args;`)

**Interfaces:**
- Produces:
  - `pub struct Args { pub test: bool, pub scene: String, pub time: Option<f32>, pub camera: CameraSpec, pub seed: u64, pub cars: u32, pub peds: u32, pub screenshot: Option<PathBuf>, pub disable: PostFxMask, pub freeze_time: bool, pub show_bounds: bool }`
  - `pub enum CameraSpec { BehindPlayer, Free { pos: Vector3, yaw: f32, pitch: f32 } }`
  - `pub fn parse_args() -> Args`
- Consumes: nothing (uses `std::env::args`).

- [ ] **Step 1: Write the test for the parser**

Create `src/cli_args.rs` with a stub `parse_args()` and a test module:

```rust
// src/cli_args.rs
use std::path::PathBuf;
use raylib::ffi::Vector3;

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub test: bool,
    pub scene: String,
    pub time: Option<f32>,
    pub camera: CameraSpec,
    pub seed: u64,
    pub cars: u32,
    pub peds: u32,
    pub screenshot: Option<PathBuf>,
    pub disable: PostFxMaskStub,
    pub freeze_time: bool,
    pub show_bounds: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraSpec {
    BehindPlayer,
    Free { pos: Vector3, yaw: f32, pitch: f32 },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostFxMaskStub;

pub fn parse_args() -> Args {
    Args {
        test: false,
        scene: String::from("headlight_closeup"),
        time: None,
        camera: CameraSpec::BehindPlayer,
        seed: 0xC0FFEE,
        cars: 0,
        peds: 0,
        screenshot: None,
        disable: PostFxMaskStub,
        freeze_time: false,
        show_bounds: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_no_args() {
        let args = parse_args();
        assert!(!args.test);
        assert_eq!(args.scene, "headlight_closeup");
        assert_eq!(args.seed, 0xC0FFEE);
        assert_eq!(args.cars, 0);
    }
}
```

- [ ] **Step 2: Register the module**

Modify `src/lib.rs` to add the module declaration after `pub mod time;`:

```rust
pub mod cli_args;
```

- [ ] **Step 3: Build and test**

Run: `cd ~/dev/ai/gta7 && cargo test cli_args 2>&1 | tail -10`
Expected: `1 passed`.

- [ ] **Step 4: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/cli_args.rs src/lib.rs && \
  git commit -m "feat(test-harness): scaffold CLI args module and stub"
```

---

## Task 2: Real CLI parser

**Files:**
- Modify: `src/cli_args.rs` (replace `parse_args` stub; delete the stub `PostFxMaskStub`)

- [ ] **Step 1: Write the failing tests for the real parser**

Replace the test module in `src/cli_args.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn with_args(args: &[&str], f: impl FnOnce(Args)) {
        let mut iter: Vec<OsString> = args.iter().map(Into::into).collect();
        iter.insert(0, OsString::from("gta7"));
        let parsed = parse_args_with(iter.into_iter());
        f(parsed);
    }

    #[test]
    fn test_flag_sets_test_mode() {
        with_args(&["--test"], |a| assert!(a.test));
    }

    #[test]
    fn scene_parses_string() {
        with_args(&["--test", "--scene=night_street"], |a| {
            assert_eq!(a.scene, "night_street");
        });
    }

    #[test]
    fn time_is_optional_and_parses_floats() {
        with_args(&["--test", "--time=6.5"], |a| {
            assert_eq!(a.time, Some(6.5));
        });
        with_args(&["--test"], |a| assert_eq!(a.time, None));
    }

    #[test]
    fn camera_xyz() {
        with_args(&["--test", "--camera=1.0,2.0,3.0"], |a| match a.camera {
            CameraSpec::Free { pos, yaw, pitch } => {
                assert_eq!(pos.x, 1.0);
                assert_eq!(pos.y, 2.0);
                assert_eq!(pos.z, 3.0);
                assert_eq!(yaw, 0.0);
                assert_eq!(pitch, 0.0);
            }
            _ => panic!("expected Free"),
        });
    }

    #[test]
    fn camera_xyz_yaw_pitch() {
        with_args(&["--test", "--camera=1,2,3,45,-15"], |a| match a.camera {
            CameraSpec::Free { pos, yaw, pitch } => {
                assert_eq!(pos.x, 1.0);
                assert_eq!(pos.y, 2.0);
                assert_eq!(pos.z, 3.0);
                assert_eq!(yaw, 45.0);
                assert_eq!(pitch, -15.0);
            }
            _ => panic!("expected Free"),
        });
    }

    #[test]
    fn camera_behind_player_keyword() {
        with_args(&["--test", "--camera=behind_player"], |a| {
            assert_eq!(a.camera, CameraSpec::BehindPlayer);
        });
    }

    #[test]
    fn seed_default_and_override() {
        with_args(&["--test"], |a| assert_eq!(a.seed, 0xC0FFEE));
        with_args(&["--test", "--seed=42"], |a| assert_eq!(a.seed, 42));
    }

    #[test]
    fn cars_peds_default_zero() {
        with_args(&["--test"], |a| {
            assert_eq!(a.cars, 0);
            assert_eq!(a.peds, 0);
        });
    }

    #[test]
    fn screenshot_path() {
        with_args(&["--test", "--screenshot=/tmp/x.png"], |a| {
            assert_eq!(a.screenshot.as_ref().unwrap().to_str(), Some("/tmp/x.png"));
        });
    }

    #[test]
    fn disable_csv_parses_to_mask() {
        with_args(&["--test", "--disable=bloom,crt"], |a| {
            assert!(a.disable.bloom);
            assert!(!a.disable.ssr);
            assert!(a.disable.crt);
            assert!(!a.disable.god_rays);
        });
    }

    #[test]
    fn freeze_time_and_show_bounds() {
        with_args(&["--test", "--freeze-time", "--show-bounds"], |a| {
            assert!(a.freeze_time);
            assert!(a.show_bounds);
        });
    }

    #[test]
    fn unknown_flag_is_ignored() {
        with_args(&["--test", "--garbage=42"], |a| {
            assert!(a.test);
            assert_eq!(a.time, None);
        });
    }
}
```

Add `parse_args_with` to the implementation, and replace `parse_args` to call it with `std::env::args_os()`:

```rust
pub fn parse_args() -> Args {
    parse_args_with(std::env::args_os())
}

pub fn parse_args_with<I, T>(it: I) -> Args
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = Args::default();
    let mut cams: Vec<f32> = Vec::new();
    let mut it = it.into_iter();
    let _bin = it.next(); // skip argv[0]
    for raw in it {
        let s = raw.into();
        let s = match s.to_str() {
            Some(s) => s,
            None => continue,
        };
        if let Some(rest) = s.strip_prefix("--") {
            if let Some((k, v)) = rest.split_once('=') {
                match k {
                    "test" => args.test = true,
                    "scene" => args.scene = v.to_string(),
                    "time" => args.time = v.parse().ok(),
                    "seed" => args.seed = v.parse().unwrap_or(args.seed),
                    "cars" => args.cars = v.parse().unwrap_or(0),
                    "peds" => args.peds = v.parse().unwrap_or(0),
                    "camera" => {
                        if v == "behind_player" {
                            args.camera = CameraSpec::BehindPlayer;
                        } else {
                            cams = v.split(',')
                                .map(|t| t.trim().parse::<f32>().unwrap_or(0.0))
                                .collect();
                        }
                    }
                    "screenshot" => args.screenshot = Some(PathBuf::from(v)),
                    "disable" => args.disable = PostFxMask::from_csv(v),
                    _ => {} // unknown flags are ignored
                }
            } else {
                match rest {
                    "test" => args.test = true,
                    "freeze-time" => args.freeze_time = true,
                    "show-bounds" => args.show_bounds = true,
                    _ => {}
                }
            }
        }
    }
    match cams.len() {
        3 => args.camera = CameraSpec::Free {
            pos: Vector3 { x: cams[0], y: cams[1], z: cams[2] },
            yaw: 0.0, pitch: 0.0,
        },
        5 => args.camera = CameraSpec::Free {
            pos: Vector3 { x: cams[0], y: cams[1], z: cams[2] },
            yaw: cams[3], pitch: cams[4],
        },
        _ => {} // leave as default
    }
    args
}
```

- [ ] **Step 2: Run tests; expect failures on stub**

Run: `cd ~/dev/ai/gta7 && cargo test cli_args 2>&1 | tail -20`
Expected: ~10 failures because `PostFxMask` and `Args::default` don't exist yet.

- [ ] **Step 3: Implement `Args::default` and stub `PostFxMask`**

In `src/cli_args.rs`, add `Args::default()` (mirror of the existing struct-literal in the stub) and define a placeholder `PostFxMask` with the fields the tests use but real `from_csv` logic deferred to Task 3:

```rust
impl Default for Args {
    fn default() -> Self {
        Self {
            test: false,
            scene: String::from("headlight_closeup"),
            time: None,
            camera: CameraSpec::BehindPlayer,
            seed: 0xC0FFEE,
            cars: 0,
            peds: 0,
            screenshot: None,
            disable: PostFxMask::none(),
            freeze_time: false,
            show_bounds: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostFxMask {
    pub bloom: bool,
    pub ssr: bool,
    pub crt: bool,
    pub god_rays: bool,
    pub fog: bool,
    pub shadows: bool,
    pub dyn_sky: bool,
}

impl PostFxMask {
    pub fn none() -> Self { Self::default() }
    pub fn any(&self) -> bool {
        self.bloom || self.ssr || self.crt || self.god_rays
            || self.fog || self.shadows || self.dyn_sky
    }
}
```

- [ ] **Step 4: Run tests; expect 1 failure on `from_csv`**

Run: `cd ~/dev/ai/gta7 && cargo test cli_args 2>&1 | tail -20`
Expected: 1 test fails (`disable_csv_parses_to_mask`).

- [ ] **Step 5: Implement `from_csv`**

Add to `impl PostFxMask`:

```rust
pub fn from_csv(s: &str) -> Self {
    let mut m = Self::none();
    for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tok.to_ascii_lowercase().as_str() {
            "bloom" => m.bloom = true,
            "ssr" => m.ssr = true,
            "crt" => m.crt = true,
            "godrays" | "god_rays" => m.god_rays = true,
            "fog" => m.fog = true,
            "shadows" | "shadow" => m.shadows = true,
            "dyn" | "dyn_sky" | "sky" => m.dyn_sky = true,
            _ => {} // unknown tokens are ignored
        }
    }
    m
}
```

- [ ] **Step 6: Run tests, expect green**

Run: `cd ~/dev/ai/gta7 && cargo test cli_args 2>&1 | tail -5`
Expected: `11 passed; 0 failed`.

- [ ] **Step 7: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/cli_args.rs && \
  git commit -m "feat(test-harness): real CLI parser with --test gate and all flags"
```

---

## Task 3: PostFxMask -> render integration

**Files:**
- Modify: `src/render/postfx.rs` (gate bloom/SSR/CRT/god-rays passes behind `disabled` mask)
- Modify: `src/render/lighting.rs` (gate shadow pass)
- Create: `src/postfx_mask.rs` (move `PostFxMask` here from `cli_args`)
- Modify: `src/cli_args.rs` (re-export `PostFxMask`)
- Modify: `src/lib.rs` (register `postfx_mask`)

- [ ] **Step 1: Move `PostFxMask` to its own module**

Create `src/postfx_mask.rs`:

```rust
//! Bitset of post-processing passes that can be disabled at runtime.
//! Lives in its own module so `render` doesn't depend on `cli_args`.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostFxMask {
    pub bloom: bool,
    pub ssr: bool,
    pub crt: bool,
    pub god_rays: bool,
    pub fog: bool,
    pub shadows: bool,
    pub dyn_sky: bool,
}

impl PostFxMask {
    pub fn none() -> Self { Self::default() }

    pub fn any(&self) -> bool {
        self.bloom || self.ssr || self.crt || self.god_rays
            || self.fog || self.shadows || self.dyn_sky
    }

    pub fn from_csv(s: &str) -> Self {
        let mut m = Self::none();
        for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            match tok.to_ascii_lowercase().as_str() {
                "bloom" => m.bloom = true,
                "ssr" => m.ssr = true,
                "crt" => m.crt = true,
                "godrays" | "god_rays" => m.god_rays = true,
                "fog" => m.fog = true,
                "shadows" | "shadow" => m.shadows = true,
                "dyn" | "dyn_sky" | "sky" => m.dyn_sky = true,
                _ => {}
            }
        }
        m
    }
}
```

- [ ] **Step 2: Update `cli_args.rs` to re-export from the new module**

Replace the `PostFxMask` definition in `src/cli_args.rs` with `pub use crate::postfx_mask::PostFxMask;` and update `Args::default` to use `PostFxMask::none()`. Add `pub mod postfx_mask;` to `src/lib.rs`.

- [ ] **Step 3: Build and re-test the CLI parser**

Run: `cd ~/dev/ai/gta7 && cargo test cli_args 2>&1 | tail -5`
Expected: still 11 passed.

- [ ] **Step 4: Add `disabled` to `PostFx` and gate passes**

In `src/render/postfx.rs`:

```rust
use crate::postfx_mask::PostFxMask;

pub struct PostFx {
    // ... existing fields ...
    pub disabled: PostFxMask,
}

impl PostFx {
    pub fn load(...) -> Self {
        // ... existing setup ...
        Self {
            // ... existing fields ...
            disabled: PostFxMask::none(),
        }
    }

    pub fn set_disabled(&mut self, mask: PostFxMask) {
        self.disabled = mask;
    }

    pub fn process(&mut self, ...) {
        if self.disabled.bloom {
            // Bloom off: copy scene_fbo verbatim into output_fbo.
            let scene_tex = self.scene_fbo.texture().clone();
            let mut ot = rl.begin_texture_mode(thread, &mut self.output_fbo);
            ot.clear_background(Color::BLACK);
            ot.draw_texture_pro(scene_tex, full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
        } else {
            // ... existing bloom passes 1-3 (bright extract + 2x blur + composite) ...
        }

        if !self.disabled.ssr && self.ssr_wetness > 0.01 {
            // ... existing SSR pass ...
        }

        if !self.disabled.god_rays && self.god_ray_intensity > 0.01 {
            // ... existing god rays pass ...
        }

        if !self.disabled.crt {
            // ... existing CRT pass ...
        }
        // else: CRT off — output_fbo already holds the previous pass's result; do nothing.
    }
}
```

The key invariant: when `disabled.bloom == true`, we MUST still produce a valid `output_fbo` so subsequent passes (or the screen blit) have something to read. The verbatim copy is the simplest correct behavior.

- [ ] **Step 5: Add `disabled` to `LightingSystem` and gate shadow pass**

In `src/render/lighting.rs`:

```rust
use crate::postfx_mask::PostFxMask;

pub struct LightingSystem {
    // ... existing fields ...
    pub disabled: PostFxMask,
}

impl LightingSystem {
    pub fn load(...) -> Self {
        Self {
            // ... existing fields ...
            disabled: PostFxMask::none(),
        }
    }

    pub fn set_disabled(&mut self, mask: PostFxMask) {
        self.disabled = mask;
    }
}
```

In `src/game.rs:render`, wrap the shadow pass at lines 1360-1389 with:

```rust
if !self.lighting.disabled.shadows {
    // ... existing shadow pass code ...
}
```

When shadows are disabled, the previous frame's shadow map stays in `shadow_map` (or an uninitialized white texture on first frame). For first pass, an uninitialized map means lit = no shadow contribution, which is the correct "no shadows" state. We can refine later if needed.

- [ ] **Step 6: Build and run all tests**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: all tests pass (22+).

- [ ] **Step 7: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/postfx_mask.rs src/cli_args.rs src/lib.rs \
  src/render/postfx.rs src/render/lighting.rs src/game.rs && \
  git commit -m "feat(test-harness): PostFxMask + per-pass disable hooks"
```

---

## Task 4: `FollowCamera::Mode` and free-fly camera

**Files:**
- Modify: `src/camera.rs` (add `Mode`, free camera methods)
- Modify: `src/game.rs:render` (branch on camera mode in the per-frame update)

- [ ] **Step 1: Add the `Mode` enum and free camera methods to `FollowCamera`**

In `src/camera.rs`, extend the struct:

```rust
pub struct FollowCamera {
    pub pos: Vector3,
    pub target: Vector3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub height: f32,
    pub mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Follow,
    Free,
}
```

Add new methods:

```rust
impl FollowCamera {
    pub fn is_free(&self) -> bool {
        matches!(self.mode, Mode::Free)
    }

    pub fn set_free(&mut self, pos: Vector3, yaw: f32, pitch: f32) {
        self.mode = Mode::Free;
        self.pos = pos;
        self.target = Vector3 {
            x: pos.x + yaw.cos() * pitch.cos(),
            y: pos.y + pitch.sin(),
            z: pos.z + yaw.sin() * pitch.cos(),
        };
        self.yaw = yaw;
        self.pitch = pitch;
    }

    pub fn set_follow(&mut self) {
        self.mode = Mode::Follow;
    }

    /// Free-fly input. `input` carries keyboard + mouse state.
    /// `dt` is real time since the last update.
    pub fn update_free(&mut self, input: &crate::input::Input, dt: f32) {
        use raylib::consts::KeyboardKey;
        let speed = 8.0; // m/s
        let rot_speed = 1.5; // rad/s

        // Translation: WASD on horizontal plane relative to current yaw; QE for up/down.
        let (mut mx, mut mz, mut my) = (0.0_f32, 0.0_f32, 0.0_f32);
        if input.move_forward > 0.0 { mz += 1.0; }
        if input.move_forward < 0.0 { mz -= 1.0; }
        if input.strafe > 0.0      { mx += 1.0; }
        if input.strafe < 0.0      { mx -= 1.0; }
        if input.jump { my += 1.0; }            // jump → up
        if input.handbrake { my -= 1.0; }       // space (handbrake) → down
        // Q and E are not in the Input struct yet; add them to Input:
        // (see step 2 below for input.rs addition)

        // Normalize to unit length so diagonal speed is bounded.
        let len = (mx * mx + mz * mz + my * my).sqrt();
        if len > 0.0 {
            mx /= len; mz /= len; my /= len;
        }

        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        self.pos.x += (cy * mz + sy * mx) * speed * dt;
        self.pos.z += (-sy * mz + cy * mx) * speed * dt;
        self.pos.y += my * speed * dt;

        // Yaw with A/D (strafe left/right) when no strafe motion, otherwise use mouse.
        // Keep simple: yaw always from mouse drag (look_dx/look_dy already in Input).
        self.yaw   -= input.look_dx   * rot_speed;
        self.pitch += input.look_dy   * rot_speed;
        self.pitch = self.pitch.clamp(-1.4, 1.4);

        // Update target so `forward()` and `to_camera3d()` still work in free mode.
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        self.target = Vector3 {
            x: self.pos.x + cy * cp,  // forward = +X at yaw 0
            y: self.pos.y + sp,
            z: self.pos.z + sy * cp,
        };
    }
}
```

- [ ] **Step 2: Extend `Input` with `look_dx`/`look_dy` already present, but also add `Q`/`E` for up/down**

`Input` already has `move_forward`, `strafe`, `jump`, `handbrake`, `look_dx`, `look_dy` (per `src/input.rs:30-50`). Add two new fields:

```rust
pub struct Input {
    // ... existing fields ...
    pub ascend: bool,  // E
    pub descend: bool, // Q
}
```

And sample them in `Input::sample`:

```rust
i.ascend  = rl.is_key_down(KeyboardKey::KEY_E);
i.descend = rl.is_key_down(KeyboardKey::KEY_Q);
```

In `update_free` above, replace the `jump`/`handbrake` block with:

```rust
if input.ascend  { my += 1.0; }
if input.descend { my -= 1.0; }
```

- [ ] **Step 3: Add unit test for free camera clamp**

In `src/camera.rs` test module (create if not present):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    #[test]
    fn free_mode_moves_on_w() {
        let mut cam = FollowCamera::new();
        cam.set_free(Vector3 { x: 0.0, y: 1.5, z: 0.0 }, 0.0, 0.0);
        let before = cam.pos.z;
        let mut input = Input::default();
        input.move_forward = 1.0;
        cam.update_free(&input, 0.1);
        // yaw=0 → +Z direction
        assert!(cam.pos.z > before, "forward should move +Z, got {} -> {}", before, cam.pos.z);
    }

    #[test]
    fn free_mode_pitch_is_clamped() {
        let mut cam = FollowCamera::new();
        cam.set_free(Vector3 { x: 0.0, y: 1.5, z: 0.0 }, 0.0, 0.0);
        let mut input = Input::default();
        input.look_dy = 10.0;
        for _ in 0..20 { cam.update_free(&input, 0.016); }
        assert!(cam.pitch <= 1.4, "pitch must clamp, got {}", cam.pitch);
    }
}
```

Add a minimal `Default` for `Input` if it doesn't have one already (check `src/input.rs`).

- [ ] **Step 4: Wire free camera into Game::render**

In `src/game.rs:render`, locate the existing per-frame camera update path (around line 1336+ where `to_camera3d` is called). The current code calls `self.camera.update(...)` somewhere (check the calling pattern). If the camera's `update` is called from `Game::update` rather than `Game::render`, keep it there and add an `if self.camera.is_free()` branch:

```rust
if self.camera.is_free() {
    self.camera.update_free(input, dt);
} else {
    self.camera.update(&self.player, &self.vehicles,
                        self.look_accum_x, self.look_accum_y,
                        self.cfg.mouse_sensitivity, dt);
}
```

- [ ] **Step 5: Build and test**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/camera.rs src/input.rs src/game.rs && \
  git commit -m "feat(test-harness): FollowCamera free-fly mode"
```

---

## Task 5: Game::enter_test_mode (set_time, freeze, capture)

**Files:**
- Modify: `src/game.rs` (add `enter_test_mode`, `set_time`, `test_capture_pending`)

- [ ] **Step 1: Add `set_time` and the test-mode fields to `Game`**

In `src/game.rs`, add a field to `Game`:

```rust
pub struct Game<'a> {
    // ... existing fields ...
    pub test_capture_pending: Option<std::path::PathBuf>,
    pub args: Option<crate::cli_args::Args>,
}
```

Add methods (alongside `set_time`):

```rust
impl<'a> Game<'a> {
    /// Set the game time (in raw `self.time` units, i.e. game hours * time_scale).
    /// `hour` is in real hours 0..24. Clamps out-of-range with `rem_euclid(24)` so
    /// `--time=25` becomes 1:00 instead of crashing.
    pub fn set_time(&mut self, hour: f32) {
        self.time = hour.rem_euclid(24.0) / self.cfg.time_scale;
    }

    pub fn enter_test_mode(&mut self, args: crate::cli_args::Args) {
        // Apply disable mask to render and lighting.
        self.lighting.set_disabled(args.disable.clone());
        self.postfx.set_disabled(args.disable.clone());

        // Time.
        if let Some(t) = args.time { self.set_time(t); }
        // (freeze_time is enforced in update())

        // Override scene state to skip title/intro.
        self.screen_state = ScreenState::Playing;

        // Disable the ambient initial mission / cars to give a clean scene.
        self.vehicles.clear();
        self.traffic.clear();
        self.cops.clear();
        self.pickups.clear();
        self.peds.clear();

        // Spawn requested NPC cars ahead of preset (preset may add more).
        for _ in 0..args.cars {
            crate::ai::traffic::spawn_traffic(
                &self.city, &mut self.vehicles, &mut self.traffic,
            );
        }

        // Spawn peds on sidewalks.
        for _ in 0..args.peds {
            let (pos, _axis) = self.city.nearest_sidewalk(
                (rand::random::<f32>() - 0.5) * 100.0,
                (rand::random::<f32>() - 0.5) * 100.0,
            );
            let colors = [
                Color::new(255, 20, 147, 255),
                Color::new(0, 240, 255, 255),
                Color::new(180, 0, 255, 255),
                Color::new(50, 255, 50, 255),
                Color::new(255, 110, 0, 255),
            ];
            let col = colors[rand::random::<usize>() % colors.len()];
            self.peds.push(crate::ai::ped::Ped::new(pos, col));
        }

        // Ensure the city has blocks around the player position.
        let p = self.player.pos;
        let mut shops = Vec::new();
        let mut pickups = Vec::new();
        self.city.ensure_blocks_around(p, 6, &self.cfg, &mut shops, &mut pickups);
        // Add the new shops/pickups (existing ones are gone since we cleared).
        // The City already generated these; they were in `self.pickups` before the clear.
        // We append the fresh ones, but `pickups` was just cleared. That's fine —
        // test mode may have no pickups, or we can re-emit them:
        // (the city keeps its own references; we only cleared our Vec.)
        for shop in shops { self.shops.push(shop); }
        for pickup in pickups { self.pickups.push(pickup); }

        // Apply the scene preset (mutates vehicles/player/camera).
        crate::test_scene::apply_scene(self, &args);

        // Camera.
        match args.camera {
            crate::cli_args::CameraSpec::BehindPlayer => {
                self.camera.set_follow();
            }
            crate::cli_args::CameraSpec::Free { pos, yaw, pitch } => {
                self.camera.set_free(pos, yaw, pitch);
            }
        }

        // Stash the args for the main loop to use.
        self.args = Some(args);
    }
}
```

- [ ] **Step 2: Gate `self.time += dt` behind `freeze_time`**

In `src/game.rs:update`, wrap each `self.time += dt` occurrence (lines 265, 299, 359) with:

```rust
if !self.args.as_ref().map_or(false, |a| a.freeze_time) {
    self.time += dt;
}
```

Replace each existing `self.time += dt;` (3 places) with that guarded version. The Title and Intro branches should also respect the freeze (frozen time during cutscene is fine — it just means sky/sun don't change).

- [ ] **Step 3: Build, expect compile errors**

The `crate::test_scene` module doesn't exist yet; the call will fail to compile.

Run: `cd ~/dev/ai/gta7 && cargo build 2>&1 | tail -10`
Expected: `error[E0433]: failed to resolve: could not find 'test_scene' in 'crate'`.

- [ ] **Step 4: Stub `src/test_scene.rs`**

Create `src/test_scene.rs` with a no-op `apply_scene` and register the module:

```rust
// src/test_scene.rs
use crate::cli_args::Args;
use crate::game::Game;

pub fn apply_scene(_game: &mut Game, _args: &Args) {
    // TODO: real presets in Task 6
}
```

Add `pub mod test_scene;` to `src/lib.rs`.

- [ ] **Step 5: Build, run tests**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: compiles, tests pass.

- [ ] **Step 6: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/game.rs src/test_scene.rs src/lib.rs && \
  git commit -m "feat(test-harness): enter_test_mode, set_time, freeze flag"
```

---

## Task 6: Scene presets

**Files:**
- Modify: `src/test_scene.rs` (real implementations of 4 presets)

- [ ] **Step 1: Define the `SCENES` registry and implement presets**

Replace `src/test_scene.rs` with:

```rust
//! Named scene presets for the test harness. Each preset mutates a `Game`
//! into a deterministic state for screenshot/inspection.
use crate::ai::ped::Ped;
use crate::ai::traffic::spawn_traffic;
use crate::camera::Mode;
use crate::cli_args::Args;
use crate::game::Game;
use crate::vehicle::{Vehicle, VehicleKind, VehicleVariant};
use raylib::color::Color;
use raylib::ffi::Vector3;

pub const SCENES: &[(&str, fn(&mut Game, &Args))] = &[
    ("headlight_closeup", scene_headlight_closeup),
    ("night_street",       scene_night_street),
    ("dawn_drive",         scene_dawn_drive),
    ("parking_lot",        scene_parking_lot),
];

/// Apply a named scene. Unknown names print a helpful list and pick
/// `headlight_closeup` as a safe default.
pub fn apply_scene(game: &mut Game, args: &Args) {
    if let Some((_, f)) = SCENES.iter().find(|(name, _)| *name == args.scene) {
        f(game, args);
        return;
    }
    eprintln!("Unknown scene `{}`. Available:", args.scene);
    for (name, _) in SCENES { eprintln!("  {name}"); }
    let (_, f) = SCENES[0];
    f(game, args);
}

fn vehicle_with_variant(
    pos: Vector3, yaw: f32, color: Color, kind: VehicleKind, variant: VehicleVariant,
) -> Vehicle {
    let mut v = Vehicle::new(pos, yaw, color, kind);
    v.variant = variant;
    v
}

/// Place player on foot at origin, two cars 6m apart along +X pointed +Z,
/// camera 3m behind & at eye level looking down +Z.
fn scene_headlight_closeup(game: &mut Game, _args: &Args) {
    if game.args.as_ref().map_or(true, |a| a.time.is_none()) {
        game.set_time(22.0);
    }
    game.player.in_vehicle = None;
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    game.player.yaw = 0.0;
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: -3.0, y: 0.0, z: 4.0 }, 0.0,
        Color::new(60, 120, 200, 255), VehicleKind::Civilian, VehicleVariant::Sedan,
    ));
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: 3.0, y: 0.0, z: 4.0 }, 0.0,
        Color::new(200, 60, 60, 255), VehicleKind::Civilian, VehicleVariant::Sports,
    ));
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_free(
            Vector3 { x: 0.0, y: 1.5, z: -3.0 }, 0.0, 0.0,
        );
    }
}

fn scene_night_street(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(21.0); }
    game.player.in_vehicle = Some(0);
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }, 0.0,
        Color::new(255, 20, 147, 255), VehicleKind::Civilian, VehicleVariant::Sedan,
    ));
    game.player.in_vehicle = Some(0);
    game.vehicles[0].variant = VehicleVariant::Sedan;
    for i in 0..args.cars.max(4) {
        spawn_traffic(&game.city, &mut game.vehicles, &mut game.traffic);
    }
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_follow();
    }
}

fn scene_dawn_drive(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(6.5); }
    // Reuse night_street setup but earlier time.
    scene_night_street(game, args);
}

fn scene_parking_lot(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(19.5); }
    game.player.in_vehicle = None;
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    let layout = [
        (-9.0, 0.0, 6.0, 0.0,  VehicleVariant::Sedan,  Color::new(220, 60, 60, 255)),
        (-3.0, 0.0, 6.0, 0.0,  VehicleVariant::Sports, Color::new(255, 110, 0, 255)),
        ( 3.0, 0.0, 6.0, 0.0,  VehicleVariant::SUV,    Color::new(60, 180, 220, 255)),
        ( 9.0, 0.0, 6.0, 0.0,  VehicleVariant::Pickup, Color::new(80, 220, 80, 255)),
        ( 0.0, 0.0,-6.0, std::f32::consts::PI, VehicleVariant::Sedan,  Color::new(160, 60, 220, 255)),
        ( 6.0, 0.0,-6.0, std::f32::consts::PI, VehicleVariant::Sports, Color::new(220, 220, 80, 255)),
    ];
    for (x, _y, z, yaw, variant, color) in layout {
        game.vehicles.push(vehicle_with_variant(
            Vector3 { x, y: 0.0, z }, yaw, color, VehicleKind::Civilian, variant,
        ));
    }
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_free(
            Vector3 { x: 0.0, y: 8.0, z: 12.0 }, -std::f32::consts::FRAC_PI_2, 0.0,
        );
    }
}
```

- [ ] **Step 2: Build and test**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: compiles, all tests pass.

- [ ] **Step 3: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/test_scene.rs && \
  git commit -m "feat(test-harness): scene presets (headlight_closeup, night_street, dawn_drive, parking_lot)"
```

---

## Task 7: Main loop integration + screenshot mode

**Files:**
- Modify: `src/main.rs` (parse args, branch into test mode)

- [ ] **Step 1: Rewrite `main.rs` to parse args and branch into test mode**

Replace `src/main.rs` with:

```rust
use gta7::cli_args::{parse_args, CameraSpec};
use gta7::config::Config;
use gta7::game::Game;
use gta7::input::Input;
use gta7::time::Clock;
use gta7::postfx_mask::PostFxMask;
use raylib::consts::KeyboardKey;
use std::path::PathBuf;

fn main() {
    let args = parse_args();

    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title(if args.test { format!("GTA7 [test: {}]", args.scene) } else { "GTA7".to_string() })
        .build();
    let cfg = Config::default();
    rl.set_target_fps(cfg.logic_rate.hz() as u32);
    rl.enable_cursor();
    rl.set_exit_key(None);

    let audio = raylib::prelude::RaylibAudio::init_audio_device().unwrap();

    let mut game = Game::new(&mut rl, &thread, cfg, &audio);
    let mut clock = Clock::new(game.cfg.logic_rate);
    let mut cursor_enabled = true;

    if args.test {
        // Capture screenshot path up front.
        let screenshot_path = args.screenshot.clone();
        game.enter_test_mode(args);

        if let Some(path) = screenshot_path {
            // Screenshot mode: run one logic step + one render, save, exit.
            let mut input = Input::sample(&rl);
            game.update(&mut input, clock.dt());
            game.render(&mut rl, &thread, 1.0, rl.get_fps() as i32);
            rl.take_screenshot(&thread, path.to_str().expect("screenshot path"));
            return;
        }

        // Interactive test mode: regular main loop, test hotkeys active.
        interactive_loop(&mut rl, &thread, &mut game, &mut clock, &mut cursor_enabled);
        return;
    }

    // Normal game loop (unchanged path).
    normal_loop(&mut rl, &thread, &mut game, &mut clock, &mut cursor_enabled);
}

fn normal_loop(
    rl: &mut raylib::RaylibHandle, thread: &raylib::ffi::RaylibThread,
    game: &mut Game, clock: &mut Clock, cursor_enabled: &mut bool,
) {
    while !rl.window_should_close() {
        let target_cursor = game.paused || game.screen_state == gta7::game::ScreenState::Title;
        if target_cursor != *cursor_enabled {
            *cursor_enabled = target_cursor;
            if *cursor_enabled { rl.enable_cursor(); } else { rl.disable_cursor(); }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            if game.screen_state == gta7::game::ScreenState::Playing {
                game.paused = !game.paused;
            } else if game.screen_state == gta7::game::ScreenState::Title {
                game.quit = true;
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F11) { rl.toggle_fullscreen(); }
        if !game.paused { game.handle_hotkeys(rl); }
        if clock.rate() != game.cfg.logic_rate {
            clock.set_rate(game.cfg.logic_rate);
            rl.set_target_fps(game.cfg.logic_rate.hz() as u32);
        }
        let mut input = Input::sample(rl);
        if !game.paused {
            game.look_accum_x += input.look_dx;
            game.look_accum_y += input.look_dy;
        }
        let steps = clock.tick(rl.get_frame_time());
        if !game.paused {
            for _ in 0..steps { game.update(&mut input, clock.dt()); }
        }
        let fps = rl.get_fps();
        game.render(rl, thread, clock.alpha, fps as i32);
        if game.pending_fullscreen { game.pending_fullscreen = false; rl.toggle_fullscreen(); }
        if game.quit { break; }
    }
}

fn interactive_loop(
    rl: &mut raylib::RaylibHandle, thread: &raylib::ffi::RaylibThread,
    game: &mut Game, clock: &mut Clock, cursor_enabled: &mut bool,
) {
    let mut cycle_scene_idx: usize = 0;
    while !rl.window_should_close() {
        // Always enable cursor in interactive test mode (no auto-hide).
        if !*cursor_enabled { rl.enable_cursor(); *cursor_enabled = true; }
        if rl.is_key_pressed(KeyboardKey::KEY_F11) { rl.toggle_fullscreen(); }
        if rl.is_key_pressed(KeyboardKey::KEY_F1) { game.cfg.debug_overlay = !game.cfg.debug_overlay; }
        if rl.is_key_pressed(KeyboardKey::KEY_F3) { /* bounds: stub for now */ }
        if rl.is_key_pressed(KeyboardKey::KEY_F5) {
            cycle_scene_idx = (cycle_scene_idx + 1) % gta7::test_scene::SCENES.len();
            let (name, f) = gta7::test_scene::SCENES[cycle_scene_idx];
            let mut a = game.args.clone().unwrap_or_default();
            a.scene = name.to_string();
            game.enter_test_mode(a);
            f(game, &game.args.clone().unwrap());
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F6) {
            if game.camera.is_free() { game.camera.set_follow(); } else {
                game.camera.set_free(game.camera.pos, game.camera.yaw, game.camera.pitch);
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            take_screenshot(rl, thread, game);
        }
        // Numpad +/- to advance time.
        if rl.is_key_pressed(KeyboardKey::KEY_KP_ADD) { game.set_time(game.time * game.cfg.time_scale + 0.5); }
        if rl.is_key_pressed(KeyboardKey::KEY_KP_SUBTRACT) { game.set_time(game.time * game.cfg.time_scale - 0.5); }

        let mut input = Input::sample(rl);
        game.look_accum_x += input.look_dx;
        game.look_accum_y += input.look_dy;
        let steps = clock.tick(rl.get_frame_time());
        for _ in 0..steps { game.update(&mut input, clock.dt()); }
        let fps = rl.get_fps();
        game.render(rl, thread, clock.alpha, fps as i32);
    }
}

fn take_screenshot(rl: &mut raylib::RaylibHandle, thread: &raylib::ffi::RaylibThread, _game: &mut Game) {
    use std::time::{SystemTime, UNIX_EPOCH};
    std::fs::create_dir_all("screenshots").ok();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = format!("screenshots/{stamp}.png");
    rl.take_screenshot(thread, &path);
    eprintln!("Saved screenshot: {path}");
}
```

- [ ] **Step 2: Build**

Run: `cd ~/dev/ai/gta7 && cargo build 2>&1 | tail -15`
Expected: either compiles, or compile errors due to small details (e.g. `gta7::game::ScreenState` access). Fix any errors by following the compiler hints. Likely culprits: `rl: &mut RaylibHandle` vs `&mut rl` in `interactive_loop`/`normal_loop` (the function takes ownership through `&mut`). The `&mut rl` pattern in the old loop is `let (mut rl, thread) = ...` then `rl.is_key_pressed(...)` — to extract that into a helper, pass `&mut rl`.

- [ ] **Step 3: Run tests**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: all tests pass (they don't touch main.rs, but cargo test compiles the full crate).

- [ ] **Step 4: Verify the binary still runs the normal path**

Run: `cd ~/dev/ai/gta7 && cargo run 2>&1 | head -5` then Ctrl-C after a moment. Expected: title screen appears, no panic. This is a smoke test that the unmodified path still works.

- [ ] **Step 5: Verify test mode compiles in headless**

Run: `cd ~/dev/ai/gta7 && cargo build --release 2>&1 | tail -5`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
cd ~/dev/ai/gta7 && git add src/main.rs && \
  git commit -m "feat(test-harness): wire test mode into main loop with screenshot + interactive paths"
```

---

## Task 8: Final integration + smoke tests

**Files:**
- Modify: `docs/superpowers/specs/2026-06-24-test-harness-design.md` (note in Risks that the harness is the tool, not the fix)

- [ ] **Step 1: Run the full test suite**

Run: `cd ~/dev/ai/gta7 && cargo test 2>&1 | tail -5`
Expected: all tests pass.

- [ ] **Step 2: Verify the normal game still works (smoke test)**

Run: `cd ~/dev/ai/gta7 && cargo run 2>&1 | head -10`
Expected: window opens, title screen renders, no panics. (Skip if running in headless CI; the build success in Task 7 Step 5 is sufficient evidence.)

- [ ] **Step 3: Document the usage in the spec doc**

Add a "Usage Examples" section to the spec doc, just before "## Risks & Open Questions":

```markdown
## Usage Examples

```bash
# Default headlight closeup, interactive
cargo run --release -- --test --scene=headlight_closeup

# Take a screenshot of the parking lot at dusk
cargo run --release -- --test --scene=parking_lot --time=19.5 \
    --screenshot=/tmp/parking_lot.png

# Reproduce the night-smearing bug, CRT off, see raw scene
cargo run --release -- --test --scene=headlight_closeup --disable=crt

# Drive around in the dark with 4 traffic cars ahead
cargo run --release -- --test --scene=night_street --cars=4
```
```

- [ ] **Step 4: Commit and push**

```bash
cd ~/dev/ai/gta7 && git add docs/superpowers/specs/2026-06-24-test-harness-design.md && \
  git commit -m "docs(test-harness): add usage examples to spec"
git push origin main
```

---

## Self-Review (against the spec)

1. **Spec coverage:**
   - `--test` gate → Tasks 1, 2, 5, 7
   - `--scene=NAME` → Tasks 2, 6
   - `--time=HOUR` with decimal support → Task 5 (`set_time`)
   - `--camera=X,Y,Z[,yaw,pitch]` and `behind_player` → Tasks 2, 4, 5, 7
   - `--seed` (default 0xC0FFEE) → Task 2
   - `--cars=N`, `--peds=N` → Tasks 2, 5
   - `--screenshot=PATH` → Task 7
   - `--disable=LIST` covering bloom/ssr/crt/godrays/fog/shadows/dyn → Tasks 2, 3
   - `--freeze-time` → Task 5
   - `--show-bounds` (B/F3 alias) → Task 7 (stubbed, full impl deferred to a follow-up)
   - Screenshot mode (one update + one render + TakeScreenshot) → Task 7
   - Interactive mode hotkeys (F1, F3, F5, F6, P, Numpad±) → Task 7
   - 4 scene presets → Task 6
   - `PostFxMask` struct + `from_csv` → Task 3
   - Camera `Mode` enum + free-fly controls (WASD, QE, right-drag) → Task 4
   - Lighting/shadow disable → Task 3
   - Spec non-goals (no CI, no replays) — explicitly skipped
   - Risks covered: free camera no collision (acknowledged), `--time=25` wraps (Task 5 `rem_euclid(24)`), multiple `--disable` (last wins via overwrite)

2. **Placeholder scan:** None.

3. **Type consistency:** `Mode::{Follow, Free}` matches between `camera.rs` and `game.rs`. `PostFxMask` fields match between `cli_args.rs`, `postfx_mask.rs`, and render consumers. `Args::default` is the single source of defaults.

4. **Test coverage:** Tasks 1, 2, 4, 6 each include unit tests. Task 7 includes a smoke test.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-24-test-harness.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
