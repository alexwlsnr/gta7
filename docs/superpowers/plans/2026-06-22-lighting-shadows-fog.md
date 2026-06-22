# Lighting, Shadows & Fog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement directional lighting, real-time shadow mapping, and distance fog using custom GLSL shaders in raylib, driven by the existing day/night cycle.

**Architecture:** Two-pass rendering — shadow map pass (orthographic sun camera → depth texture), then lit scene pass with custom shader (directional light + ambient + shadow sampling + exponential fog). A new `LightingSystem` struct manages shaders, shadow map render texture, and per-frame uniforms.

**Tech Stack:** Rust, raylib 6.0, GLSL 330 (OpenGL 3.3), raylib `RenderTexture2D`, `Shader`, `begin_shader_mode` / `begin_texture_mode` RAII guards, `Matrix::look_at` / `Matrix::ortho` for light space matrix.

## Global Constraints

- Shaders are GLSL version 330 core (matches raylib's OpenGL 3.3 backend)
- Shadow map resolution: 1024×1024
- Only buildings, vehicles, and characters cast shadows (roads/sidewalks are receivers only)
- Fog must not affect HUD or pause menu (2D overlay drawn after shader mode ends)
- Sun direction and color derived from game time via `sky_colors_for_hour` and a new `sun_direction` function
- Shaders loaded from files in `assets/shaders/`
- Must compile with zero warnings on all three CI platforms (Linux/macOS/Windows)
- Existing tests must continue to pass

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `assets/shaders/lighting.vs` | Create | Main vertex shader: outputs world pos, normal, view dir |
| `assets/shaders/lighting.fs` | Create | Main fragment shader: directional light + ambient + shadow + fog |
| `assets/shaders/depth.vs` | Create | Shadow pass vertex shader: minimal transform |
| `assets/shaders/depth.fs` | Create | Shadow pass fragment shader: output depth |
| `src/render/lighting.rs` | Create | `LightingSystem` struct: shaders, shadow map, uniforms, sun computation |
| `src/lib.rs` | Modify | Add `pub mod render::lighting` |
| `src/config.rs` | Modify | Add `sun_direction(hour)` and `sun_color(hour)` functions |
| `src/render/models.rs` | Modify | Accept `&LightingSystem` in draw functions for shadow pass rendering |
| `src/game.rs` | Modify | Orchestrate shadow pass + lit pass in `render()` |
| `.gitignore` | Modify | Ensure `assets/shaders/` is NOT ignored |

---

### Task 1: Sun Position & Color Computation

**Files:**
- Modify: `src/config.rs` (after existing `sky_colors_for_hour` function)
- Test: `src/config.rs` inline `#[cfg(test)]` module

**Interfaces:**
- Consumes: `sky_colors_for_hour(hour: f32) -> (Color, Color)` (already exists)
- Produces:
  - `sun_direction(hour: f32) -> Vector3` — normalized direction FROM the sun (points toward scene)
  - `sun_color(hour: f32) -> Color` — sun light color at given hour
  - `sun_position(hour: f32, player_pos: Vector3) -> Vector3` — sun world position for shadow camera

- [ ] **Step 1: Write failing tests for sun direction and color**

Add to the end of `src/config.rs`, before the closing `}` of the test module (if one exists at file level) or as a new test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use raylib::ffi::Vector3;

    #[test]
    fn sun_direction_at_noon_is_downward() {
        let dir = sun_direction(13.0);
        // At noon (13h), sun should be roughly overhead — direction pointing down.
        assert!(dir.y < -0.5, "sun should point downward at noon, got y={}", dir.y);
    }

    #[test]
    fn sun_direction_at_midnight_is_dim() {
        let dir = sun_direction(0.0);
        // At midnight, sun is below horizon — direction pointing up (moonlight from below).
        assert!(dir.y > 0.0, "sun should point upward at midnight (below horizon), got y={}", dir.y);
    }

    #[test]
    fn sun_color_at_noon_is_bright() {
        let col = sun_color(13.0);
        assert!(col.r > 200 && col.g > 200 && col.b > 180,
            "noon sun should be bright white, got {:?}", col);
    }

    #[test]
    fn sun_color_at_dusk_is_warm() {
        let col = sun_color(18.5);
        // Dusk should be warm — more red than blue.
        assert!(col.r > col.b, "dusk sun should be warmer (r > b), got r={} b={}", col.r, col.b);
    }

    #[test]
    fn sun_color_at_night_is_dim() {
        let col = sun_color(0.0);
        // Night sun (moonlight) should be very dim.
        assert!(col.r < 80 && col.g < 80 && col.b < 100,
            "night sun should be dim, got {:?}", col);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config`
Expected: FAIL with "function `sun_direction` not found"

- [ ] **Step 3: Implement sun_direction, sun_color, and sun_position**

Add to `src/config.rs` after the existing `format_game_time` function:

```rust
use raylib::ffi::Vector3;

/// Compute the sun direction (normalized vector pointing FROM the sun toward the scene)
/// for a given hour (0..24). At noon the sun is overhead (direction = down).
/// At night the direction flips (moonlight from below horizon).
pub fn sun_direction(hour: f32) -> Vector3 {
    let h = hour.rem_euclid(24.0);
    // Sun angle: 0 at midnight (below), PI at noon (overhead).
    // Map hour to angle: midnight=0, noon=PI. So angle = (h/24) * TAU, but shifted so noon = PI.
    let angle = ((h - 6.0) / 24.0) * std::f32::consts::TAU; // 6h = sunrise = angle 0
    // Sun arcs east→west. X component sweeps from -1 (east, morning) to +1 (west, afternoon).
    // Y component: sin(angle) — positive = above horizon, negative = below.
    let x = angle.cos();
    let y = angle.sin();
    let z = 0.3; // Slight tilt so shadows aren't purely along one axis.
    let len = (x * x + y * y + z * z).sqrt();
    Vector3 { x: x / len, y: y / len, z: z / len }
}

/// Compute the sun/moon light color for a given hour.
/// Warm at dawn/dusk, white at noon, dim cool moonlight at night.
pub fn sun_color(hour: f32) -> Color {
    let h = hour.rem_euclid(24.0);
    let keyframes: [(f32, Color); 6] = [
        (0.0,  Color::new(30, 35, 55, 255)),    // midnight — dim moonlight
        (6.0,  Color::new(120, 80, 60, 255)),   // pre-dawn — dim warm
        (7.5,  Color::new(255, 180, 120, 255)), // dawn — warm orange
        (13.0, Color::new(255, 250, 235, 255)), // noon — bright white
        (18.5, Color::new(255, 160, 90, 255)),  // dusk — warm orange
        (24.0, Color::new(30, 35, 55, 255)),    // wraps to midnight
    ];
    let mut i = 0;
    while i < keyframes.len() - 1 && keyframes[i + 1].0 <= h {
        i += 1;
    }
    let (t0, c0) = keyframes[i];
    let (t1, c1) = keyframes[i + 1];
    let t = if t1 > t0 { (h - t0) / (t1 - t0) } else { 0.0 };
    Color::new(
        (c0.r as f32 + (c1.r as f32 - c0.r as f32) * t) as u8,
        (c0.g as f32 + (c1.g as f32 - c0.g as f32) * t) as u8,
        (c0.b as f32 + (c1.b as f32 - c0.b as f32) * t) as u8,
        255,
    )
}

/// Compute the sun's world position for shadow camera placement.
/// `dir` = sun direction (from sun_direction). `player_pos` = camera target.
/// Sun is placed far along the inverse direction.
pub fn sun_position(dir: Vector3, player_pos: Vector3) -> Vector3 {
    // Sun is 200 units away in the opposite direction of the light.
    Vector3 {
        x: player_pos.x - dir.x * 200.0,
        y: player_pos.y - dir.y * 200.0,
        z: player_pos.z - dir.z * 200.0,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config`
Expected: PASS — all 5 new tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: sun direction and color computation from game time"
```

---

### Task 2: GLSL Shader Files

**Files:**
- Create: `assets/shaders/lighting.vs`
- Create: `assets/shaders/lighting.fs`
- Create: `assets/shaders/depth.vs`
- Create: `assets/shaders/depth.fs`

**Interfaces:**
- Produces: shader files consumed by `LightingSystem::load()` in Task 3

- [ ] **Step 1: Create the depth vertex shader**

Create `assets/shaders/depth.vs`:

```glsl
#version 330 core

in vec3 vertexPosition;

uniform mat4 mvp;

void main() {
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
```

- [ ] **Step 2: Create the depth fragment shader**

Create `assets/shaders/depth.fs`:

```glsl
#version 330 core

layout(location = 0) out vec4 fragColor;

void main() {
    // Output linearized depth in red channel (enough for shadow comparison).
    float depth = gl_FragCoord.z;
    fragColor = vec4(depth, 0.0, 0.0, 1.0);
}
```

- [ ] **Step 3: Create the lighting vertex shader**

Create `assets/shaders/lighting.vs`:

```glsl
#version 330 core

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 model;
uniform mat4 lightSpaceMatrix;

out vec3 fragWorldPos;
out vec2 fragTexCoord;
out vec3 fragNormal;
out vec4 fragLightSpacePos;

void main() {
    vec4 worldPos = model * vec4(vertexPosition, 1.0);
    fragWorldPos = worldPos.xyz;
    fragTexCoord = vertexTexCoord;
    fragNormal = mat3(model) * vertexNormal;
    fragLightSpacePos = lightSpaceMatrix * worldPos;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
```

- [ ] **Step 4: Create the lighting fragment shader**

Create `assets/shaders/lighting.fs`:

```glsl
#version 330 core

in vec3 fragWorldPos;
in vec2 fragTexCoord;
in vec3 fragNormal;
in vec4 fragLightSpacePos;

uniform vec3 u_lightDir;
uniform vec3 u_lightColor;
uniform vec3 u_ambientColor;
uniform vec3 u_fogColor;
uniform float u_fogDensity;
uniform vec3 u_cameraPos;
uniform sampler2D u_shadowMap;
uniform sampler2D texture0;
uniform vec4 u_colDiffuse;

out vec4 finalColor;

float compute_shadow() {
    // Perspective divide.
    vec3 projCoords = fragLightSpacePos.xyz / fragLightSpacePos.w;
    projCoords = projCoords * 0.5 + 0.5;
    // Outside shadow map bounds — no shadow.
    if (projCoords.x < 0.0 || projCoords.x > 1.0 ||
        projCoords.y < 0.0 || projCoords.y > 1.0 ||
        projCoords.z > 1.0) {
        return 1.0; // fully lit
    }
    float closestDepth = texture(u_shadowMap, projCoords.xy).r;
    float currentDepth = projCoords.z;
    // Shadow bias to prevent acne.
    float bias = 0.005;
    return currentDepth - bias < closestDepth ? 1.0 : 0.4;
}

void main() {
    // Sample texture.
    vec4 texColor = texture(texture0, fragTexCoord);
    vec3 baseColor = texColor.rgb * u_colDiffuse.rgb;

    // Normalize vectors.
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(-u_lightDir);

    // Diffuse lighting.
    float diff = max(dot(normal, lightDir), 0.0);
    // Soft wrap so back faces aren't fully dark.
    float wrap = max(dot(normal, lightDir) * 0.5 + 0.5, 0.0);
    wrap = wrap * wrap;

    // Shadow factor.
    float shadow = compute_shadow();

    // Combine: ambient + (diffuse * shadow * light color).
    vec3 ambient = u_ambientColor * baseColor;
    vec3 diffuse = u_lightColor * baseColor * diff * shadow;
    vec3 fill = u_ambientColor * baseColor * wrap * 0.5;
    vec3 lit = ambient + diffuse + fill;

    // Exponential fog.
    float dist = length(u_cameraPos - fragWorldPos);
    float fogFactor = 1.0 - exp(-u_fogDensity * dist);
    fogFactor = clamp(fogFactor, 0.0, 1.0);

    vec3 final = mix(lit, u_fogColor, fogFactor);
    finalColor = vec4(final, u_colDiffuse.a);
}
```

- [ ] **Step 5: Commit**

```bash
git add assets/shaders/
git commit -m "feat: GLSL shaders for shadow mapping and lit scene rendering"
```

---

### Task 3: LightingSystem Module

**Files:**
- Create: `src/render/lighting.rs`
- Modify: `src/lib.rs` — add `pub mod render` if not present, then `pub mod lighting` inside

**Interfaces:**
- Consumes:
  - `crate::config::{sun_direction, sun_color, sun_position, sky_colors_for_hour}`
  - `raylib::prelude::*` (RaylibHandle, RaylibThread, Shader, RenderTexture2D, Camera3D, Vector3, Matrix, Color)
  - `raylib::ffi::{Camera3D, Vector3}`
- Produces:
  - `pub struct LightingSystem` with fields:
    - `pub lit_shader: Shader` — main lighting shader
    - `pub depth_shader: Shader` — shadow pass shader
    - `pub shadow_map: RenderTexture2D`
    - `pub shadow_camera: Camera3D`
    - Uniform locations cached as `i32` fields
  - `pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self`
  - `pub fn begin_shadow_pass(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, player_pos: Vector3, hour: f32)`
  - `pub fn end_shadow_pass(&self, rl: &mut RaylibHandle, thread: &RaylibThread)`
  - `pub fn update_uniforms(&mut self, hour: f32, sky_bottom: Color, camera_pos: Vector3)`
  - `pub fn begin_lit_mode(&mut self, d: &mut impl RaylibDraw3D) -> Option<ShaderMode3D>`
  - `pub fn get_light_space_matrix(&self) -> Matrix`

- [ ] **Step 1: Create the LightingSystem struct and load function**

Create `src/render/lighting.rs`:

```rust
//! Lighting system: shadow mapping + directional light + fog via custom shaders.
use raylib::prelude::*;
use raylib::ffi::{Vector3, Camera3D};
use raylib::consts::CameraProjection;
use crate::config::{sun_direction, sun_color, sun_position};

pub struct LightingSystem {
    pub lit_shader: Shader,
    pub depth_shader: Shader,
    pub shadow_map: RenderTexture2D,
    pub shadow_camera: Camera3D,
    // Cached uniform locations for lit shader.
    loc_light_dir: i32,
    loc_light_color: i32,
    loc_ambient_color: i32,
    loc_fog_color: i32,
    loc_fog_density: i32,
    loc_camera_pos: i32,
    loc_shadow_map: i32,
    loc_light_space: i32,
    // Cached uniform locations for depth shader.
    loc_depth_mvp: i32,
    // Current light space matrix (updated each frame).
    light_space: Matrix,
}

impl LightingSystem {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let lit_shader = rl.load_shader(
            thread,
            Some("assets/shaders/lighting.vs"),
            Some("assets/shaders/lighting.fs"),
        ).unwrap_or_else(|_| rl.load_shader(thread, None, None).unwrap());

        let depth_shader = rl.load_shader(
            thread,
            Some("assets/shaders/depth.vs"),
            Some("assets/shaders/depth.fs"),
        ).unwrap_or_else(|_| rl.load_shader(thread, None, None).unwrap());

        let shadow_map = rl.load_render_texture(thread, 1024, 1024).unwrap();

        // Shadow camera: orthographic, follows player.
        let shadow_camera = Camera3D {
            position: Vector3 { x: 0.0, y: 200.0, z: 0.0 },
            target: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            up: Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            fovy: 120.0,
            projection: CameraProjection::CAMERA_ORTHOGRAPHIC,
        };

        // Cache uniform locations.
        let loc_light_dir = lit_shader.get_shader_location("u_lightDir");
        let loc_light_color = lit_shader.get_shader_location("u_lightColor");
        let loc_ambient_color = lit_shader.get_shader_location("u_ambientColor");
        let loc_fog_color = lit_shader.get_shader_location("u_fogColor");
        let loc_fog_density = lit_shader.get_shader_location("u_fogDensity");
        let loc_camera_pos = lit_shader.get_shader_location("u_cameraPos");
        let loc_shadow_map = lit_shader.get_shader_location("u_shadowMap");
        let loc_light_space = lit_shader.get_shader_location("u_lightSpaceMatrix");
        let loc_depth_mvp = depth_shader.get_shader_location("mvp");

        LightingSystem {
            lit_shader,
            depth_shader,
            shadow_map,
            shadow_camera,
            loc_light_dir,
            loc_light_color,
            loc_ambient_color,
            loc_fog_color,
            loc_fog_density,
            loc_camera_pos,
            loc_shadow_map,
            loc_light_space,
            loc_depth_mvp,
            light_space: Matrix::identity(),
        }
    }

    /// Compute the light space matrix (view + projection) for shadow mapping.
    fn compute_light_space_matrix(&self) -> Matrix {
        let view = Matrix::look_at(
            self.shadow_camera.position,
            self.shadow_camera.target,
            self.shadow_camera.up,
        );
        let aspect = 1.0;
        let proj = Matrix::ortho(
            -self.shadow_camera.fovy as f64 / 2.0,
            self.shadow_camera.fovy as f64 / 2.0,
            -self.shadow_camera.fovy as f64 / 2.0,
            self.shadow_camera.fovy as f64 / 2.0,
            1.0,
            400.0,
        );
        Matrix::multiply(proj, view)
    }

    /// Begin the shadow map render pass. Call before rendering shadow casters.
    pub fn begin_shadow_pass(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, player_pos: Vector3, hour: f32) {
        let dir = sun_direction(hour);
        let sun_pos = sun_position(dir, player_pos);

        // Update shadow camera to follow player, positioned at sun.
        self.shadow_camera.position = sun_pos;
        self.shadow_camera.target = player_pos;

        // Compute and store light space matrix.
        self.light_space = self.compute_light_space_matrix();

        // Begin rendering to shadow map.
        let mut d = rl.begin_texture_mode(thread, &mut self.shadow_map);
        d.clear_background(Color::new(255, 255, 255, 255)); // Far = white (no shadow).
        // Clear depth buffer.
        d.clear_background(Color::new(255, 255, 255, 255));

        // Set up depth shader.
        let mvp = Matrix::multiply(self.light_space, Matrix::identity());
        self.depth_shader.set_shader_value_matrix(self.loc_depth_mvp, mvp);
    }

    /// End the shadow map render pass.
    pub fn end_shadow_pass(&self) {
        // RAII guard handles this automatically when `d` drops.
        // This function is a no-op placeholder for API symmetry.
    }

    /// Update lighting uniforms for the current frame.
    pub fn update_uniforms(&mut self, hour: f32, sky_bottom: Color, camera_pos: Vector3) {
        let dir = sun_direction(hour);
        let sun_col = sun_color(hour);

        // Light direction (from sun toward scene).
        self.lit_shader.set_shader_value(self.loc_light_dir, dir);
        // Light color (0..1 range).
        self.lit_shader.set_shader_value(self.loc_light_color, Vector3 {
            x: sun_col.r as f32 / 255.0,
            y: sun_col.g as f32 / 255.0,
            z: sun_col.b as f32 / 255.0,
        });
        // Ambient = sky bottom color (dimmed).
        self.lit_shader.set_shader_value(self.loc_ambient_color, Vector3 {
            x: sky_bottom.r as f32 / 255.0 * 0.4,
            y: sky_bottom.g as f32 / 255.0 * 0.4,
            z: sky_bottom.b as f32 / 255.0 * 0.4,
        });
        // Fog color = sky bottom.
        self.lit_shader.set_shader_value(self.loc_fog_color, Vector3 {
            x: sky_bottom.r as f32 / 255.0,
            y: sky_bottom.g as f32 / 255.0,
            z: sky_bottom.b as f32 / 255.0,
        });
        // Fog density: higher at night for atmosphere.
        let is_night = hour.rem_euclid(24.0) < 6.0 || hour.rem_euclid(24.0) > 20.0;
        let density = if is_night { 0.012 } else { 0.006 };
        self.lit_shader.set_shader_value(self.loc_fog_density, density);
        // Camera position for fog distance.
        self.lit_shader.set_shader_value(self.loc_camera_pos, camera_pos);
        // Light space matrix for shadow lookup.
        self.lit_shader.set_shader_value_matrix(self.loc_light_space, self.light_space);
        // Shadow map texture.
        self.lit_shader.set_shader_value_texture(self.loc_shadow_map, &self.shadow_map.texture());
    }

    /// Get the light space matrix (for use in draw functions if needed).
    pub fn get_light_space_matrix(&self) -> Matrix {
        self.light_space
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

Read `src/lib.rs` and add `pub mod render` if not present. Inside `src/render/mod.rs` (or wherever the render module is declared), add:

```rust
pub mod lighting;
```

Check the existing module structure — if `src/render/` is already a directory with `mod.rs`, add there. If not, check `src/lib.rs` for the existing `pub mod render` declaration.

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors. If `Matrix::multiply` doesn't exist, use the `*` operator or `raylib_sys::MatrixMultiply`. Check the raylib-rs API — `Matrix` implements `std::ops::Mul`.

- [ ] **Step 4: Commit**

```bash
git add src/render/lighting.rs src/lib.rs src/render/mod.rs
git commit -m "feat: LightingSystem module — shader loading, shadow map, uniform management"
```

---

### Task 4: Integrate Lighting Into Game Render Pipeline

**Files:**
- Modify: `src/game.rs` — `render()` method and `Game::new()`
- Modify: `src/render/models.rs` — shadow pass draw functions
- Modify: `src/game.rs` — add `lighting: LightingSystem` field to `Game` struct

**Interfaces:**
- Consumes:
  - `crate::render::lighting::LightingSystem` from Task 3
  - `crate::config::{sun_direction, sun_color, sky_colors_for_hour}` from Task 1
  - All existing draw functions in `src/render/models.rs`
- Produces:
  - Modified `Game` struct with `lighting: LightingSystem` field
  - Modified `render()` method that does shadow pass → lit pass → HUD

- [ ] **Step 1: Add LightingSystem to Game struct and initialize in new()**

In `src/game.rs`, add the field to the `Game` struct:

```rust
pub lighting: crate::render::lighting::LightingSystem,
```

In `Game::new()`, after `let sfx = ...`, add:

```rust
let lighting = crate::render::lighting::LightingSystem::load(rl, thread);
```

And in the `Game { ... }` initializer, add:

```rust
lighting,
```

- [ ] **Step 2: Modify render() to do shadow pass + lit pass**

Replace the existing 3D scene rendering block in `render()` with the two-pass approach. The key change is:

1. Before `begin_drawing()`, run the shadow pass:
   - Call `self.lighting.begin_shadow_pass(rl, thread, player_pos, hour)`
   - Render shadow casters (buildings, vehicles, characters) using depth shader
   - The RAII guard from `begin_texture_mode` auto-closes when dropped

2. In the main 3D scene block, wrap all 3D draws with `begin_shader_mode`:
   - Call `self.lighting.update_uniforms(hour, sky_bottom, cam_pos)` first
   - Then wrap the 3D drawing calls in `d.begin_shader_mode(&mut self.lighting.lit_shader)`

The shadow pass rendering requires drawing all shadow-casting geometry with the depth shader. Create a helper function in `models.rs`:

```rust
/// Draw all shadow-casting geometry for the shadow map pass.
pub fn draw_shadow_casters(
    d3: &mut impl RaylibDraw3D,
    city: &City,
    assets: &Assets,
    cfg: &Config,
    vehicles: &[Vehicle],
    peds: &[Ped],
    cops: &[Cop],
    player: &Player,
) {
    // Buildings.
    for b in &city.buildings {
        let c = b.box3d.center();
        let h = b.box3d.half();
        d3.draw_cube(c, h.x * 2.0, h.y * 2.0, h.z * 2.0, Color::WHITE);
    }
    // Vehicles (simple boxes for shadow).
    for v in vehicles {
        if v.destroyed { continue; }
        d3.draw_cube(v.pos, 2.0, 0.8, 4.2, Color::WHITE);
    }
    // Characters (simple capsules for shadow).
    for ped in peds {
        if ped.dead() { continue; }
        d3.draw_cube(ped.pos, 0.4, 1.8, 0.4, Color::WHITE);
    }
    for cop in cops {
        if cop.dead() { continue; }
        d3.draw_cube(cop.pos, 0.4, 1.8, 0.4, Color::WHITE);
    }
    // Player.
    if player.alive {
        d3.draw_cube(player.pos, 0.4, 1.8, 0.4, Color::WHITE);
    }
}
```

Note: The shadow pass needs a 3D mode camera. The `begin_shadow_pass` function should set up the shadow camera in 3D mode. The implementation needs to handle the raylib RAII borrow rules — `begin_texture_mode` and `begin_mode3D` both borrow the draw handle.

The actual implementation in `render()` will look like:

```rust
// --- Shadow Pass ---
{
    let hour = (self.time * self.cfg.time_scale).rem_euclid(24.0);
    let player_pos = self.player.pos;
    self.lighting.begin_shadow_pass(rl, thread, player_pos, hour);
    // Render shadow casters inside the texture mode.
    // This requires careful handling of raylib's RAII borrow chains.
    // The shadow pass draws simplified geometry (boxes) with the depth shader.
}
```

The exact borrow structure depends on raylib-rs's API. The key insight: `begin_texture_mode` returns a draw handle, and `begin_mode3D` can be called on that handle. The shadow camera is used for the 3D mode.

- [ ] **Step 3: Wrap main 3D scene in shader mode**

In the existing 3D scene block, after `let mut d3 = d.begin_mode3D(cam);`, wrap the draw calls:

```rust
// Update lighting uniforms before drawing.
self.lighting.update_uniforms(total_hours, sky_bottom, cam_pos);

// Begin shader mode for lit rendering.
{
    let mut d3s = d3.begin_shader_mode(&mut self.lighting.lit_shader);
    // All existing 3D draw calls go here, using d3s instead of d3.
    draw_world(&mut d3s, &self.city, &self.assets, &self.cfg);
    // ... vehicles, peds, cops, player, fx ...
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles. May need to fix borrow checker issues with the RAII guards. The key pattern is to chain: `rl.begin_texture_mode(thread, &mut shadow_map)` → `d.begin_mode3D(shadow_cam)` → `d.begin_shader_mode(&mut depth_shader)`.

- [ ] **Step 5: Run existing tests**

Run: `cargo test`
Expected: All 22+ tests pass (lighting doesn't affect logic tests).

- [ ] **Step 6: Commit**

```bash
git add src/game.rs src/render/models.rs
git commit -m "feat: integrate two-pass lighting pipeline — shadow map + lit scene"
```

---

### Task 5: Fog Tuning & Night Lighting

**Files:**
- Modify: `src/render/lighting.rs` — fog density and ambient levels
- Modify: `assets/shaders/lighting.fs` — fog mix tuning

**Interfaces:**
- Consumes: All from Task 3 and Task 4
- Produces: Final tuned visual output

- [ ] **Step 1: Tune fog density for daytime vs nighttime**

In `src/render/lighting.rs`, `update_uniforms()`:

```rust
// Fog density: higher at night for atmosphere.
let h = hour.rem_euclid(24.0);
let density = if h < 6.0 || h > 20.0 {
    0.015  // Night: thicker fog
} else if h < 8.0 || h > 18.0 {
    0.010  // Dawn/dusk: moderate
} else {
    0.005  // Day: light fog
};
self.lit_shader.set_shader_value(self.loc_fog_density, density);
```

- [ ] **Step 2: Tune ambient light levels**

In `src/render/lighting.rs`, `update_uniforms()`, adjust ambient multiplier based on time:

```rust
// Ambient = sky bottom color, dimmed more at night.
let h = hour.rem_euclid(24.0);
let ambient_mult = if h < 6.0 || h > 20.0 {
    0.2  // Night: low ambient
} else if h < 8.0 || h > 18.0 {
    0.3  // Dawn/dusk
} else {
    0.45 // Day: brighter ambient
};
self.lit_shader.set_shader_value(self.loc_ambient_color, Vector3 {
    x: sky_bottom.r as f32 / 255.0 * ambient_mult,
    y: sky_bottom.g as f32 / 255.0 * ambient_mult,
    z: sky_bottom.b as f32 / 255.0 * ambient_mult,
});
```

- [ ] **Step 3: Build and run**

Run: `cargo build && cargo test`
Expected: Compiles, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/render/lighting.rs assets/shaders/lighting.fs
git commit -m "tune: fog density and ambient levels for day/night cycle"
```

---

### Task 6: Final Verification & CI

**Files:**
- Verify: All files compile clean
- Verify: All tests pass
- Verify: Shaders are committed and not gitignored

- [ ] **Step 1: Full clean build**

Run: `cargo clean && cargo build`
Expected: Clean build, no warnings.

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Verify shader files are tracked**

Run: `git ls-files assets/shaders/`
Expected: 4 shader files listed.

- [ ] **Step 4: Push and verify CI**

```bash
git push origin main
```

Watch CI at https://github.com/alexwlsnr/gta7/actions — all three platforms should pass.

- [ ] **Step 5: Final commit if any fixes needed**

If CI reveals issues (e.g., shader compilation on Windows, path issues), fix and re-push.
