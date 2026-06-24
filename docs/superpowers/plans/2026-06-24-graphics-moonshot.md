# Graphics Moonshot: Post-Processing Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a multi-pass post-processing pipeline (bloom, CRT filter, starfield/sky dome, god rays, SSR) that takes the vaporwave neon city's graphics to the moon.

**Architecture:** Render 3D scene to an offscreen FBO, then chain fullscreen shader passes (bright extract → blur → bloom composite → SSR → god rays → CRT post) before outputting to screen. A PostFx struct in `src/render/postfx.rs` owns all FBOs, shaders, and pass orchestration. The existing shadow pass and lit shader remain unchanged.

**Tech Stack:** Rust, raylib 6.0, GLSL 330 (OpenGL 3.3), raylib `RenderTexture2D` for FBOs, `begin_texture_mode` + `begin_shader_mode` + `draw_texture_pro` for fullscreen passes.

## Global Constraints

- Must work on OpenGL 3.3 / GLSL 330 (raylib's default backend)
- Must maintain 60fps at 1280×720 on RTX 5080
- Must not regress to black geometry or break HUD/pause menu
- Must keep all existing tests passing
- Shaders loaded from files in `assets/shaders/` (not inline)
- Each pass verified individually before moving to the next
- raylib Rust API: `rl.begin_texture_mode(thread, &mut fbo)` for FBO render, `d.begin_shader_mode(&mut shader)` for shader, `d.draw_texture_pro(tex, src_rect, dest_rect, origin, 0.0, WHITE)` for fullscreen quad
- Screen size is 1280×720 (from `src/main.rs:9`)

---

## Task 1: PostFx Scaffold — Scene FBO + Blit

**Files:**
- Create: `src/render/postfx.rs`
- Modify: `src/render/mod.rs` — add `pub mod postfx;`
- Modify: `src/game.rs:1333-1822` — redirect 3D rendering to scene_fbo, blit to screen

**Interfaces:**
- Produces: `PostFx` struct with `load(rl, thread, width, height) -> Self`, `begin_scene(rl, thread) -> RaylibDrawHandle` (draw 3D into scene_fbo), `apply(d: &mut RaylibDrawHandle)` (blit scene_fbo to screen for now)
- Consumes: `rl.begin_texture_mode(thread, &mut self.scene_fbo)` from raylib API

**What this proves:** The FBO pipeline works. The scene renders identically through an offscreen texture. No visual change from current state.

- [ ] **Step 1: Create `src/render/postfx.rs` with PostFx struct**

```rust
//! Post-processing pipeline: renders scene to FBO, chains fullscreen shader passes.
use raylib::prelude::*;

pub struct PostFx {
    pub scene_fbo: RenderTexture2D,
    width: i32,
    height: i32,
}

impl PostFx {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let scene_fbo = rl.load_render_texture(thread, width, height).unwrap();
        // Set scene FBO texture filter to bilinear for smoother post-fx sampling.
        scene_fbo.texture().set_filter(rl, TextureFilter::FILTER_BILINEAR);
        Self { scene_fbo, width, height }
    }

    /// Begin rendering the 3D scene into the offscreen FBO.
    /// Returns a draw handle — caller draws 3D world into it, then drops it.
    pub fn begin_scene<'a>(
        &'a mut self,
        rl: &'a mut RaylibHandle,
        thread: &'a RaylibThread,
    ) -> impl RaylibDraw3D + 'a {
        let mut dt = rl.begin_texture_mode(thread, &mut self.scene_fbo);
        dt.clear_background(Color::BLACK);
        // Use a 3D camera mode — caller will pass camera via a separate method.
        // For now, return the 2D draw handle so caller can begin_mode3D themselves.
        dt
    }

    /// Blit the scene FBO to the screen. In this scaffold, just copies it.
    /// Later tasks will replace this with the full post-processing chain.
    pub fn apply(&self, d: &mut RaylibDrawHandle) {
        let src = Rectangle::new(0.0, 0.0, self.width as f32, -self.height as f32);
        let dst = Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32);
        d.draw_texture_pro(
            self.scene_fbo.texture(),
            src,
            dst,
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
    }
}
```

Note: The `begin_scene` method returns a `RaylibTextureMode` draw handle. The caller (game.rs) will call `dt.begin_mode3D(cam)` on it to get a 3D draw handle. The negative height in `src` is because raylib FBO textures are upside-down — this flips them right-side up.

- [ ] **Step 2: Add module declaration in `src/render/mod.rs`**

Add `pub mod postfx;` after the existing modules:

```rust
pub mod models;
pub mod fx;
pub mod lighting;
pub mod postfx;
```

- [ ] **Step 3: Add `postfx` field to `Game` struct and load it in `Game::new`**

In `src/game.rs`, add to the `Game` struct (near the `lighting` field at line ~47):

```rust
pub postfx: crate::render::postfx::PostFx,
```

In `Game::new` (after lighting is loaded, around line 58-59), add:

```rust
let postfx = crate::render::postfx::PostFx::load(rl, thread, 1280, 720);
```

And in the `Game { ... }` initializer, add `postfx,` after `lighting,`.

- [ ] **Step 4: Modify `render()` to use PostFx**

In `src/game.rs::render()`, replace the current `let mut d = rl.begin_drawing(thread);` block (line 1388) with:

1. Render the 3D scene into `scene_fbo` instead of directly to screen
2. Then `begin_drawing` and blit the FBO to screen via `postfx.apply()`
3. Then draw HUD on top (existing code stays)

The key change is wrapping the existing 3D draw calls in `begin_texture_mode` on `scene_fbo` instead of `begin_drawing`:

```rust
// --- Scene Pass (to FBO) ---
{
    let mut dt = rl.begin_texture_mode(thread, &mut self.postfx.scene_fbo);
    dt.clear_background(sky_bottom);
    // Sky gradient (existing CPU-drawn bands, temporarily kept here)
    // ... existing sky band code moved here ...
    
    // Update lit shader uniforms
    self.lighting.update_uniforms(total_hours, sky_bottom, cam_pos);
    
    // 3D scene
    {
        let mut d3 = dt.begin_mode3D(cam);
        draw_world(&mut d3, &self.city, &self.assets, &self.cfg);
        // ... all existing 3D draw calls ...
    }
}

// --- Blit + HUD (to screen) ---
let mut d = rl.begin_drawing(thread);
self.postfx.apply(&mut d);
// ... existing HUD code stays ...
```

The sky band drawing (lines 1393-1411) and all 3D draw calls (lines 849-960) move inside the `begin_texture_mode` block. The HUD code (lines 961-1821) stays in `begin_drawing`.

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 6: Runtime screenshot — verify identical output**

Run the game, capture a screenshot. Expected: scene looks identical to before (same colors, same geometry). The FBO blit is a 1:1 copy at this stage.

Run: `cargo run` then capture with `import -window <id> /tmp/gta7-scaffold.png`

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: PostFx scaffold — render scene to FBO and blit to screen

Adds src/render/postfx.rs with PostFx struct managing a scene render
texture. render() now draws 3D into the offscreen FBO, then blits to
screen. No visual change — proves the FBO pipeline works before adding
post-processing passes."
```

---

## Task 2: Bloom — Bright Extract + Blur + Composite

**Files:**
- Create: `assets/shaders/bright_extract.fs`
- Create: `assets/shaders/blur.fs`
- Create: `assets/shaders/bloom_composite.fs`
- Modify: `src/render/postfx.rs` — add bloom FBOs, shaders, and pass logic

**Interfaces:**
- Produces: `PostFx.apply()` now runs bloom passes instead of a simple blit
- Consumes: `scene_fbo.texture()` as input to bright extract

**Visual result:** Bright neon lights, window glow, headlights, and explosions glow and bleed into surrounding pixels.

- [ ] **Step 1: Write `assets/shaders/bright_extract.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;
uniform float u_threshold;  // default 0.7
uniform float u_softKnee;   // default 0.3

out vec4 finalColor;

void main() {
    vec3 color = texture(texture0, fragTexCoord).rgb;
    float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
    if (luminance > u_threshold) {
        float contribution = (luminance - u_threshold) / u_softKnee;
        finalColor = vec4(color * contribution, 1.0);
    } else {
        finalColor = vec4(0.0, 0.0, 0.0, 1.0);
    }
}
```

- [ ] **Step 2: Write `assets/shaders/blur.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;
uniform vec2 u_direction;  // texel step direction, e.g. (1/width, 0) for horizontal

out vec4 finalColor;

// 9-tap Gaussian blur weights (sigma ~3.0)
const float weights[9] = float[](
    0.013, 0.041, 0.095, 0.168, 0.212, 0.168, 0.095, 0.041, 0.013
);

void main() {
    vec3 sum = vec3(0.0);
    for (int i = 0; i < 9; i++) {
        vec2 offset = u_direction * float(i - 4);
        sum += texture(texture0, fragTexCoord + offset).rgb * weights[i];
    }
    finalColor = vec4(sum, 1.0);
}
```

- [ ] **Step 3: Write `assets/shaders/bloom_composite.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;       // scene
uniform sampler2D texture1;       // bloom
uniform float u_bloomStrength;    // default 1.2

out vec4 finalColor;

void main() {
    vec3 scene = texture(texture0, fragTexCoord).rgb;
    vec3 bloom = texture(texture1, fragTexCoord).rgb;
    vec3 result = scene + bloom * u_bloomStrength;
    // Soft tone curve to prevent blowout
    result = result / (1.0 + result * 0.3);
    finalColor = vec4(result, 1.0);
}
```

- [ ] **Step 4: Add bloom FBOs and shaders to PostFx struct**

Add to the `PostFx` struct:

```rust
pub struct PostFx {
    pub scene_fbo: RenderTexture2D,
    bright_fbo: RenderTexture2D,       // half-res
    blur_fbo: [RenderTexture2D; 2],    // half-res ping-pong
    composite_fbo: RenderTexture2D,    // full-res

    bright_shader: Shader,
    blur_shader: Shader,
    bloom_shader: Shader,

    // Uniform locations
    loc_threshold: i32,
    loc_soft_knee: i32,
    loc_blur_direction: i32,
    loc_bloom_strength: i32,
    loc_bloom_scene: i32,
    loc_bloom_bloom: i32,

    width: i32,
    height: i32,
    half_width: i32,
    half_height: i32,
}
```

In `PostFx::load`, after creating `scene_fbo`, add:

```rust
let half_width = width / 2;
let half_height = height / 2;
let bright_fbo = rl.load_render_texture(thread, half_width, half_height).unwrap();
let blur_fbo = [
    rl.load_render_texture(thread, half_width, half_height).unwrap(),
    rl.load_render_texture(thread, half_width, half_height).unwrap(),
];
let composite_fbo = rl.load_render_texture(thread, width, height).unwrap();

bright_fbo.texture().set_filter(rl, TextureFilter::FILTER_BILINEAR);
blur_fbo[0].texture().set_filter(rl, TextureFilter::FILTER_BILINEAR);
blur_fbo[1].texture().set_filter(rl, TextureFilter::FILTER_BILINEAR);

let bright_shader = rl.load_shader(thread, None, Some("assets/shaders/bright_extract.fs"));
let blur_shader = rl.load_shader(thread, None, Some("assets/shaders/blur.fs"));
let bloom_shader = rl.load_shader(thread, None, Some("assets/shaders/bloom_composite.fs"));

// Cache uniform locations
let loc_threshold = bright_shader.get_shader_location("u_threshold");
let loc_soft_knee = bright_shader.get_shader_location("u_softKnee");
let loc_blur_direction = blur_shader.get_shader_location("u_direction");
let loc_bloom_strength = bloom_shader.get_shader_location("u_bloomStrength");
let loc_bloom_scene = bloom_shader.get_shader_location("texture0");
let loc_bloom_bloom = bloom_shader.get_shader_location("texture1");
```

Set default uniform values:

```rust
bright_shader.set_shader_value(loc_threshold, 0.7f32);
bright_shader.set_shader_value(loc_soft_knee, 0.3f32);
bloom_shader.set_shader_value(loc_bloom_strength, 1.2f32);
```

- [ ] **Step 5: Implement bloom passes in `PostFx::apply`**

Replace the simple blit in `apply` with:

```rust
pub fn apply(&mut self, d: &mut RaylibDrawHandle) {
    let full_src = Rectangle::new(0.0, 0.0, self.width as f32, -self.height as f32);
    let full_dst = Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32);
    let half_src = Rectangle::new(0.0, 0.0, self.half_width as f32, -self.half_height as f32);
    let half_dst = Rectangle::new(0.0, 0.0, self.half_width as f32, self.half_height as f32);

    // Pass 2: Bright extract (scene_fbo -> bright_fbo)
    {
        let mut bt = d.begin_texture_mode(&mut self.bright_fbo);
        bt.clear_background(Color::BLACK);
        {
            let mut bs = bt.begin_shader_mode(&mut self.bright_shader);
            bs.draw_texture_pro(self.scene_fbo.texture(), full_src, half_dst, Vector2::zero(), 0.0, Color::WHITE);
        }
    }

    // Pass 3-4: Blur H then V, two iterations
    for iteration in 0..2 {
        // Horizontal: blur_fbo[0] from bright_fbo (iter 0) or blur_fbo[1] (iter 1)
        let (src_idx, dst_idx) = if iteration == 0 { (0, 0) } else { (1, 0) };
        let src_tex = if iteration == 0 { &self.bright_fbo } else { &self.blur_fbo[1] };
        self.blur_shader.set_shader_value(self.loc_blur_direction, Vector2::new(1.0 / self.half_width as f32, 0.0));
        {
            let mut bt = d.begin_texture_mode(&mut self.blur_fbo[dst_idx]);
            bt.clear_background(Color::BLACK);
            {
                let mut bs = bt.begin_shader_mode(&mut self.blur_shader);
                bs.draw_texture_pro(src_tex.texture(), half_src, half_dst, Vector2::zero(), 0.0, Color::WHITE);
            }
        }
        // Vertical: blur_fbo[1] from blur_fbo[0]
        self.blur_shader.set_shader_value(self.loc_blur_direction, Vector2::new(0.0, 1.0 / self.half_height as f32));
        {
            let mut bt = d.begin_texture_mode(&mut self.blur_fbo[1]);
            bt.clear_background(Color::BLACK);
            {
                let mut bs = bt.begin_shader_mode(&mut self.blur_shader);
                bs.draw_texture_pro(self.blur_fbo[0].texture(), half_src, half_dst, Vector2::zero(), 0.0, Color::WHITE);
            }
        }
    }

    // Pass 5: Bloom composite (scene + bloom -> composite_fbo)
    {
        let mut ct = d.begin_texture_mode(&mut self.composite_fbo);
        ct.clear_background(Color::BLACK);
        {
            let mut cs = ct.begin_shader_mode(&mut self.bloom_shader);
            // Bind scene to texture slot 0, bloom to slot 1
            cs.set_shader_value_texture(self.loc_bloom_scene, self.scene_fbo.texture());
            cs.set_shader_value_texture(self.loc_bloom_bloom, self.blur_fbo[1].texture());
            cs.draw_texture_pro(self.scene_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
        }
    }

    // Blit composite to screen
    d.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
}
```

Note: The `begin_texture_mode` and `begin_shader_mode` borrow patterns need to match raylib's RAII guards. The exact borrow structure may need adjustment during implementation — the key is: FBO target guard → shader guard → draw_texture_pro → drop shader guard → drop FBO guard.

- [ ] **Step 6: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 7: Runtime screenshot — verify bloom glow**

Run the game, capture a screenshot. Expected: neon lights, window glow, headlights, and explosions have a visible glow/bloom halo. Colors should be slightly brighter in bloom-affected areas.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: bloom post-processing — bright extract + Gaussian blur + composite

Adds bright_extract.fs, blur.fs, and bloom_composite.fs shaders.
PostFx now runs: scene -> bright extract (half-res) -> 2x H+V blur ->
bloom composite (additive, strength 1.2) -> screen. Neon lights,
headlights, and explosions now glow and bleed into surrounding pixels."
```

---

## Task 3: CRT Post Filter — Chromatic Aberration + Scanlines + Vignette + ACES

**Files:**
- Create: `assets/shaders/crt_post.fs`
- Modify: `src/render/postfx.rs` — add crt_shader, run CRT pass as final output

**Interfaces:**
- Produces: `PostFx::apply()` now ends with CRT pass instead of raw composite blit
- Consumes: `composite_fbo.texture()` as input

**Visual result:** Subtle RGB channel separation, CRT scanlines, edge vignette, ACES tone mapping, film grain. The vaporwave aesthetic filter.

- [ ] **Step 1: Write `assets/shaders/crt_post.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;
uniform float u_time;
uniform vec2 u_resolution;

out vec4 finalColor;

// ACES filmic tone mapping approximation
vec3 aces(vec3 x) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

void main() {
    vec2 uv = fragTexCoord;
    vec2 center = vec2(0.5);
    vec2 dir = uv - center;

    // Chromatic aberration — RGB channels sampled at offset UVs
    float caStrength = 0.002;
    float r = texture(texture0, uv + dir * caStrength).r;
    float g = texture(texture0, uv).g;
    float b = texture(texture0, uv - dir * caStrength).b;
    vec3 color = vec3(r, g, b);

    // Scanlines — subtle CRT line modulation
    float scanline = sin(uv.y * u_resolution.y * 3.14159) * 0.04;
    color -= scanline;

    // Vignette — radial darkening at edges
    float vig = 1.0 - 0.3 * length(dir);
    color *= vig;

    // ACES tone mapping
    color = aces(color);

    // Film grain — hash-based noise
    float grain = fract(sin(dot(uv, vec2(12.9898, 78.233)) + u_time) * 43758.5453);
    color += (grain - 0.5) * 0.02;

    finalColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
```

- [ ] **Step 2: Add CRT shader to PostFx**

Add to struct:

```rust
crt_shader: Shader,
loc_crt_time: i32,
loc_crt_resolution: i32,
```

In `load`:

```rust
let crt_shader = rl.load_shader(thread, None, Some("assets/shaders/crt_post.fs"));
let loc_crt_time = crt_shader.get_shader_location("u_time");
let loc_crt_resolution = crt_shader.get_shader_location("u_resolution");
crt_shader.set_shader_value(loc_crt_resolution, Vector2::new(width as f32, height as f32));
```

- [ ] **Step 3: Replace final blit with CRT pass in `apply`**

At the end of `apply`, replace:

```rust
d.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
```

With:

```rust
// Pass 8: CRT post filter (composite -> screen)
self.crt_shader.set_shader_value(self.loc_crt_time, get_time());
{
    let mut cs = d.begin_shader_mode(&mut self.crt_shader);
    cs.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
}
```

Note: `get_time()` can use `raylib::ffi::GetTime()` or a frame counter. Use `rl.get_time()` if available, else accumulate a float from frame deltas.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 5: Runtime screenshot — verify CRT effect**

Run the game, capture a screenshot. Expected: subtle RGB fringing at edges, faint horizontal scanlines, darker corners (vignette), film grain texture. Overall image should look slightly "retro CRT" but not overwhelming.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: CRT post filter — chromatic aberration, scanlines, vignette, ACES, grain

Adds crt_post.fs as the final post-processing pass before HUD.
Subtle RGB channel separation, CRT scanline modulation, edge vignette,
ACES filmic tone mapping, and animated film grain complete the
vaporwave aesthetic."
```

---

## Task 4: Starfield + Sky Dome

**Files:**
- Create: `assets/shaders/sky.vs`
- Create: `assets/shaders/sky.fs`
- Modify: `src/render/models.rs` — add starfield texture + sky dome model to Assets
- Modify: `src/game.rs` — replace CPU sky bands with sky dome draw, add starfield draw

**Interfaces:**
- Produces: `Assets.sky_dome_model`, `Assets.starfield_tex`
- Consumes: `sky_colors_for_hour(hour)` from `src/config.rs`

**Visual result:** Smooth gradient sky from a dome mesh (no more CPU bands), with procedural stars at night that fade in/out with time of day.

- [ ] **Step 1: Write `assets/shaders/sky.vs`**

```glsl
#version 330 core

in vec3 vertexPosition;

uniform mat4 mvp;
uniform mat4 matModel;

out vec3 fragWorldDir;

void main() {
    vec4 worldPos = matModel * vec4(vertexPosition, 1.0);
    fragWorldDir = normalize(vertexPosition);
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
```

- [ ] **Step 2: Write `assets/shaders/sky.fs`**

```glsl
#version 330 core

in vec3 fragWorldDir;

uniform vec3 u_skyTop;
uniform vec3 u_skyBottom;
uniform sampler2D texture0;    // starfield
uniform float u_starAlpha;     // 0 = day, 1 = night

out vec4 finalColor;

void main() {
    float t = clamp(fragWorldDir.y * 0.5 + 0.5, 0.0, 1.0);
    vec3 skyColor = mix(u_skyBottom, u_skyTop, t);

    // Sample starfield using direction as spherical UV
    vec2 starUV = vec2(
        atan(fragWorldDir.z, fragWorldDir.x) / 6.28318 + 0.5,
        fragWorldDir.y * 0.5 + 0.5
    );
    vec4 stars = texture(texture0, starUV);
    skyColor += stars.rgb * stars.a * u_starAlpha;

    finalColor = vec4(skyColor, 1.0);
}
```

- [ ] **Step 3: Add starfield texture + sky dome model to Assets**

In `src/render/models.rs`, add to the `Assets` struct:

```rust
pub sky_dome_model: Model,
pub starfield_tex: Texture2D,
```

In `Assets::load`, after the existing texture generation:

```rust
// --- Starfield texture ---
let mut star_img = Image::gen_image_color(512, 512, Color::new(0, 0, 0, 0));
for _ in 0..1300 {  // ~0.5% density
    let x = rand::random::<i32>() % 512;
    let y = rand::random::<i32>() % 512;
    let brightness = 100 + (rand::random::<u32>() % 155) as u8;
    // 10% chance of neon-tinted star
    let color = if rand::random::<u32>() % 10 == 0 {
        match rand::random::<u32>() % 3 {
            0 => Color::new(255, 100, 200, brightness),  // pink
            1 => Color::new(100, 200, 255, brightness),  // cyan
            _ => Color::new(200, 150, 255, brightness),  // purple
        }
    } else {
        Color::new(brightness, brightness, brightness, brightness)
    };
    star_img.draw_pixel(x, y, color);
}
let starfield_tex = rl.load_texture_from_image(thread, &star_img).unwrap();

// --- Sky dome model ---
let sky_mesh = Mesh::gen_mesh_sphere(thread, 500.0, 16, 16);
let sky_weak = unsafe { sky_mesh.make_weak() };
let mut sky_dome_model = rl.load_model_from_mesh(thread, sky_weak).unwrap();
sky_dome_model.materials_mut()[0].set_material_texture(
    MaterialMapIndex::MATERIAL_MAP_ALBEDO, &starfield_tex
);
```

Add `sky_dome_model` and `starfield_tex` to the Assets initializer.

- [ ] **Step 4: Add sky shader to PostFx or LightingSystem**

Add the sky shader to `PostFx` (it's a render-time shader, not a material shader):

```rust
sky_shader: Shader,
loc_sky_top: i32,
loc_sky_bottom: i32,
loc_star_alpha: i32,
```

In `load`:

```rust
let sky_shader = rl.load_shader(thread, Some("assets/shaders/sky.vs"), Some("assets/shaders/sky.fs"));
let loc_sky_top = sky_shader.get_shader_location("u_skyTop");
let loc_sky_bottom = sky_shader.get_shader_location("u_skyBottom");
let loc_star_alpha = sky_shader.get_shader_location("u_starAlpha");
```

- [ ] **Step 5: Replace CPU sky bands with sky dome in `render()`**

In the scene FBO block (inside `begin_texture_mode`), before the 3D world draw:

1. Remove the `for y in (0..sh).step_by(2)` sky band loop (lines ~1393-1411)
2. Add sky dome draw:

```rust
// Sky dome — rendered with depth test disabled so it's always behind world
{
    // Set sky shader uniforms
    let (sky_top, sky_bottom) = crate::config::sky_colors_for_hour(total_hours);
    self.postfx.sky_shader.set_shader_value(
        self.postfx.loc_sky_top,
        Vector3::new(sky_top.r as f32 / 255.0, sky_top.g as f32 / 255.0, sky_top.b as f32 / 255.0),
    );
    self.postfx.sky_shader.set_shader_value(
        self.postfx.loc_sky_bottom,
        Vector3::new(sky_bottom.r as f32 / 255.0, sky_bottom.g as f32 / 255.0, sky_bottom.b as f32 / 255.0),
    );
    // Star alpha: 1 at night, 0 during day
    let h = total_hours.rem_euclid(24.0);
    let star_alpha = if h < 5.5 || h > 19.0 { 1.0 } else if h < 7.0 { (7.0 - h) / 1.5 } else if h > 18.0 { (h - 18.0) / 1.0 } else { 0.0 };
    self.postfx.sky_shader.set_shader_value(self.postfx.loc_star_alpha, star_alpha.clamp(0.0, 1.0));

    // Draw sky dome centered on camera, no depth write
    let sky_pos = Vector3 { x: cam_pos.x, y: 0.0, z: cam_pos.z };
    // Use a temporary mode with depth test disabled — raylib doesn't have a direct
    // API for this, so draw the dome first before any world geometry. Since it's
    // at radius 500 and the camera is inside, it will be behind everything.
    let mut ss = dt.begin_shader_mode(&mut self.postfx.sky_shader);
    ss.draw_model(&self.assets.sky_dome_model, sky_pos, 1.0, Color::WHITE);
}
```

Note: The sky dome is centered on the camera's XZ position so it follows the player. Since it's at radius 500 and the camera near plane is small, it will always be behind world geometry. We draw it first in the scene FBO before clearing depth for world geometry (or we can rely on depth testing since the dome is far away).

- [ ] **Step 6: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 7: Runtime screenshot — verify sky dome + stars**

Capture two screenshots:
1. During day (13:00) — smooth sky gradient, no visible bands, no stars
2. At night (0:00) — dark sky with visible stars, some neon-tinted

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: sky dome + procedural starfield replaces CPU sky bands

Adds sky.vs/sky.fs shaders for GPU-computed sky gradient from view
direction. Starfield is a 512×512 procedural texture with ~0.5% star
density, including neon-tinted stars. Stars fade in at night and
disappear during day. Replaces ~360 CPU draw_rectangle calls with a
single mesh draw."
```

---

## Task 5: God Rays

**Files:**
- Create: `assets/shaders/god_rays.fs`
- Modify: `src/render/postfx.rs` — add god_rays_shader and pass
- Modify: `src/game.rs` — pass sun screen position to PostFx
- Modify: `src/config.rs` — add `god_ray_intensity(hour)` function

**Interfaces:**
- Produces: `god_ray_intensity(hour: f32) -> f32` in config.rs
- Consumes: sun screen-space position (computed from camera + sun_position)

**Visual result:** Light shafts radiating from the sun at dawn/dusk, visible through the city.

- [ ] **Step 1: Add `god_ray_intensity` to `src/config.rs`**

```rust
/// God ray intensity from sun elevation. Zero at noon and night, peak at dawn/dusk.
pub fn god_ray_intensity(hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    if h < 5.0 || h > 20.0 {
        0.0
    } else {
        // Sun elevation = sin((h - 6) / 24 * TAU)
        let angle = ((h - 6.0) / 24.0) * std::f32::consts::TAU;
        let elevation = angle.sin().abs(); // 0 at horizon, 1 at peak
        // Max intensity when sun is near horizon (low elevation)
        (1.0 - elevation).max(0.0).min(1.0) * 0.6
    }
}
```

- [ ] **Step 2: Write `assets/shaders/god_rays.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;       // composite scene
uniform vec2 u_sunScreenPos;      // sun position in screen UV space (0..1)
uniform float u_intensity;        // 0..1, driven by sun elevation

out vec4 finalColor;

void main() {
    vec3 base = texture(texture0, fragTexCoord).rgb;
    if (u_intensity < 0.01) {
        finalColor = vec4(base, 1.0);
        return;
    }

    vec2 dir = u_sunScreenPos - fragTexCoord;
    float decay = 0.96;
    float density = 1.0;

    vec3 accumulation = vec3(0.0);
    float totalWeight = 0.0;

    const int SAMPLES = 32;
    for (int i = 0; i < SAMPLES; i++) {
        float t = float(i) / float(SAMPLES);
        vec2 samplePos = fragTexCoord + dir * t * density;
        float weight = pow(decay, float(i));
        accumulation += texture(texture0, samplePos).rgb * weight;
        totalWeight += weight;
    }
    accumulation /= totalWeight;

    // Additive blend with intensity
    vec3 result = base + accumulation * u_intensity * 0.3;
    finalColor = vec4(result, 1.0);
}
```

- [ ] **Step 3: Add god rays shader and pass to PostFx**

Add to struct:

```rust
god_rays_shader: Shader,
loc_gr_sun_pos: i32,
loc_gr_intensity: i32,
```

In `load`:

```rust
let god_rays_shader = rl.load_shader(thread, None, Some("assets/shaders/god_rays.fs"));
let loc_gr_sun_pos = god_rays_shader.get_shader_location("u_sunScreenPos");
let loc_gr_intensity = god_rays_shader.get_shader_location("u_intensity");
```

- [ ] **Step 4: Add god rays pass in `apply`**

After the bloom composite pass and before the CRT pass, add:

```rust
// Pass 7: God rays (additive radial blur from sun position)
if self.god_ray_intensity > 0.01 {
    self.god_rays_shader.set_shader_value(self.loc_gr_sun_pos, self.sun_screen_pos);
    self.god_rays_shader.set_shader_value(self.loc_gr_intensity, self.god_ray_intensity);
    {
        let mut gs = d.begin_shader_mode(&mut self.god_rays_shader);
        gs.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
        // Read back into composite_fbo for the CRT pass
    }
    // Need to blit the god-ray result back into composite_fbo
    // Or: run god rays into a temp FBO, then blit to composite_fbo
}
```

Note: God rays need to read from composite_fbo and write back to it. Use a blit: run god rays into `ssr_fbo` (reused as temp), then blit `ssr_fbo` back to `composite_fbo`. Alternatively, add a dedicated `godrays_fbo`.

A simpler approach: run god rays into `scene_fbo` (no longer needed after bloom), then composite_fbo = scene_fbo result. But scene_fbo has depth attached, which is unnecessary. Better to reuse `ssr_fbo` as a temp:

```rust
// God rays into ssr_fbo (temp), then blit back to composite_fbo
{
    let mut gt = d.begin_texture_mode(&mut self.ssr_fbo);
    gt.clear_background(Color::BLACK);
    {
        let mut gs = gt.begin_shader_mode(&mut self.god_rays_shader);
        gs.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
    }
}
// Blit ssr_fbo back to composite_fbo
{
    let mut ct = d.begin_texture_mode(&mut self.composite_fbo);
    ct.draw_texture_pro(self.ssr_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
}
```

- [ ] **Step 5: Pass sun screen position and intensity from game.rs**

In `render()`, before calling `postfx.apply()`, compute sun screen position:

```rust
let sun_world_pos = crate::config::sun_position(total_hours, player_pos);
let sun_screen = rl.get_world_to_screen(sun_world_pos, cam);
// Convert to UV space (0..1)
let sun_uv = Vector2::new(
    sun_screen.x / 1280.0,
    sun_screen.y / 720.0,
);
let gr_intensity = crate::config::god_ray_intensity(total_hours);
self.postfx.set_god_rays(sun_uv, gr_intensity);
```

Add a setter to PostFx:

```rust
pub fn set_god_rays(&mut self, sun_pos: Vector2, intensity: f32) {
    self.sun_screen_pos = sun_pos;
    self.god_ray_intensity = intensity;
}
```

Add fields: `sun_screen_pos: Vector2` and `god_ray_intensity: f32` to PostFx.

- [ ] **Step 6: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 7: Runtime screenshot — verify god rays at dawn/dusk**

Set game time to dawn (~6.5h) or dusk (~18.5h) and capture a screenshot. Expected: visible light shafts radiating from the sun position through the city. At noon, no god rays visible.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: volumetric god rays — radial light scattering at dawn/dusk

Adds god_rays.fs with 32-sample radial blur from sun screen position.
Intensity driven by sun elevation — zero at noon, peak at dawn/dusk,
zero at night. Light shafts visible through the city when sun is near
horizon."
```

---

## Task 6: Screen-Space Reflections

**Files:**
- Create: `assets/shaders/ssr.fs`
- Modify: `src/render/postfx.rs` — add ssr_shader and pass
- Modify: `src/game.rs` — pass camera matrices to PostFx for SSR

**Interfaces:**
- Produces: `PostFx::set_ssr_data(view_matrix, proj_matrix, camera_pos, wetness)`
- Consumes: `scene_fbo.depth()` for depth reconstruction, `composite_fbo.texture()` for color sampling

**Visual result:** Wet/metallic surfaces reflect neon lights and buildings above them. Roads at night show neon reflections.

- [ ] **Step 1: Write `assets/shaders/ssr.fs`**

```glsl
#version 330 core

in vec2 fragTexCoord;

uniform sampler2D texture0;       // composite color
uniform sampler2D texture1;       // scene depth
uniform mat4 u_proj;              // projection matrix
uniform mat4 u_invViewProj;       // inverse view-projection matrix
uniform vec3 u_cameraPos;
uniform float u_wetness;          // 0..1, road reflectivity
uniform vec2 u_resolution;

out vec4 finalColor;

// Reconstruct world position from screen UV + depth
vec3 worldPosFromDepth(vec2 uv, float depth) {
    vec4 clipPos = vec4(uv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
    vec4 worldPos = u_invViewProj * clipPos;
    return worldPos.xyz / worldPos.w;
}

void main() {
    vec3 base = texture(texture0, fragTexCoord).rgb;
    float depth = texture(texture1, fragTexCoord).r;

    // Skip sky (depth = 1.0 = far plane)
    if (depth >= 0.999) {
        finalColor = vec4(base, 1.0);
        return;
    }

    vec3 worldPos = worldPosFromDepth(fragTexCoord, depth);
    vec3 viewDir = normalize(u_cameraPos - worldPos);

    // Estimate normal from depth derivatives (screen-space)
    vec2 texelSize = 1.0 / u_resolution;
    float depthL = texture(texture1, fragTexCoord - vec2(texelSize.x, 0.0)).r;
    float depthR = texture(texture1, fragTexCoord + vec2(texelSize.x, 0.0)).r;
    float depthU = texture(texture1, fragTexCoord - vec2(0.0, texelSize.y)).r;
    float depthD = texture(texture1, fragTexCoord + vec2(0.0, texelSize.y)).r;

    vec3 posL = worldPosFromDepth(fragTexCoord - vec2(texelSize.x, 0.0), depthL);
    vec3 posR = worldPosFromDepth(fragTexCoord + vec2(texelSize.x, 0.0), depthR);
    vec3 posU = worldPosFromDepth(fragTexCoord - vec2(0.0, texelSize.y), depthU);
    vec3 posD = worldPosFromDepth(fragTexCoord + vec2(0.0, texelSize.y), depthD);

    vec3 normal = normalize(cross(posR - posL, posU - posD));

    // Fresnel — more reflection at grazing angles
    float fresnel = pow(1.0 - max(dot(normal, viewDir), 0.0), 3.0);

    // Reflectivity: high for downward-facing surfaces (roads), modulated by wetness
    float groundFactor = clamp(-normal.y * 2.0, 0.0, 1.0);  // 1 for flat ground
    float reflectivity = groundFactor * u_wetness * 0.5;

    if (reflectivity < 0.01) {
        finalColor = vec4(base, 1.0);
        return;
    }

    // Ray-march in screen space
    vec3 reflDir = reflect(-viewDir, normal);
    vec3 rayStart = worldPos;
    vec3 rayEnd = rayStart + reflDir * 50.0;

    // Project ray end to screen space
    vec4 startClip = u_proj * vec4(rayStart, 1.0);
    vec4 endClip = u_proj * vec4(rayEnd, 1.0);
    vec2 startScreen = startClip.xy / startClip.w;
    vec2 endScreen = endClip.xy / endClip.w;
    vec2 rayDir = endScreen - startScreen;

    vec3 reflection = vec3(0.0);
    float hitWeight = 0.0;
    const int STEPS = 24;
    for (int i = 1; i <= STEPS; i++) {
        float t = float(i) / float(STEPS);
        vec2 sampleUV = fragTexCoord + rayDir * t * 0.5;
        if (sampleUV.x < 0.0 || sampleUV.x > 1.0 || sampleUV.y < 0.0 || sampleUV.y > 1.0) break;

        float sampleDepth = texture(texture1, sampleUV).r;
        vec3 sampleWorld = worldPosFromDepth(sampleUV, sampleDepth);
        vec3 rayPos = mix(rayStart, rayEnd, t);

        if (distance(sampleWorld, rayPos) < 1.0 && sampleDepth < 0.999) {
            reflection = texture(texture0, sampleUV).rgb;
            hitWeight = 1.0 - t;
            break;
        }
    }

    if (hitWeight > 0.0) {
        vec3 result = mix(base, reflection, fresnel * reflectivity * hitWeight);
        finalColor = vec4(result, 1.0);
    } else {
        finalColor = vec4(base, 1.0);
    }
}
```

- [ ] **Step 2: Add SSR shader and pass to PostFx**

Add to struct:

```rust
ssr_shader: Shader,
loc_ssr_proj: i32,
loc_ssr_inv_view_proj: i32,
loc_ssr_camera_pos: i32,
loc_ssr_wetness: i32,
loc_ssr_resolution: i32,
loc_ssr_scene_color: i32,
loc_ssr_scene_depth: i32,
// Data set per frame
ssr_proj: Matrix,
ssr_inv_view_proj: Matrix,
ssr_camera_pos: Vector3,
ssr_wetness: f32,
```

In `load`:

```rust
let ssr_shader = rl.load_shader(thread, None, Some("assets/shaders/ssr.fs"));
let loc_ssr_proj = ssr_shader.get_shader_location("u_proj");
let loc_ssr_inv_view_proj = ssr_shader.get_shader_location("u_invViewProj");
let loc_ssr_camera_pos = ssr_shader.get_shader_location("u_cameraPos");
let loc_ssr_wetness = ssr_shader.get_shader_location("u_wetness");
let loc_ssr_resolution = ssr_shader.get_shader_location("u_resolution");
let loc_ssr_scene_color = ssr_shader.get_shader_location("texture0");
let loc_ssr_scene_depth = ssr_shader.get_shader_location("texture1");
ssr_shader.set_shader_value(loc_ssr_resolution, Vector2::new(width as f32, height as f32));
```

Add setter:

```rust
pub fn set_ssr_data(&mut self, proj: Matrix, inv_view_proj: Matrix, camera_pos: Vector3, wetness: f32) {
    self.ssr_proj = proj;
    self.ssr_inv_view_proj = inv_view_proj;
    self.ssr_camera_pos = camera_pos;
    self.ssr_wetness = wetness;
}
```

- [ ] **Step 3: Add SSR pass in `apply`**

After god rays (which writes to `composite_fbo`), before CRT:

```rust
// Pass 6: SSR (composite -> ssr_fbo)
{
    self.ssr_shader.set_shader_value_matrix(self.loc_ssr_proj, self.ssr_proj);
    self.ssr_shader.set_shader_value_matrix(self.loc_ssr_inv_view_proj, self.ssr_inv_view_proj);
    self.ssr_shader.set_shader_value(self.loc_ssr_camera_pos, self.ssr_camera_pos);
    self.ssr_shader.set_shader_value(self.loc_ssr_wetness, self.ssr_wetness);
    self.ssr_shader.set_shader_value_texture(self.loc_ssr_scene_depth, self.scene_fbo.depth());

    let mut st = d.begin_texture_mode(&mut self.ssr_fbo);
    st.clear_background(Color::BLACK);
    {
        let mut ss = st.begin_shader_mode(&mut self.ssr_shader);
        ss.set_shader_value_texture(self.loc_ssr_scene_color, self.composite_fbo.texture());
        ss.draw_texture_pro(self.composite_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
    }
}
// Blit ssr_fbo to composite_fbo
{
    let mut ct = d.begin_texture_mode(&mut self.composite_fbo);
    ct.draw_texture_pro(self.ssr_fbo.texture(), full_src, full_dst, Vector2::zero(), 0.0, Color::WHITE);
}
```

Note: `scene_fbo.depth()` — check if raylib's `RenderTexture2D` exposes depth texture. If not, the scene FBO may need to be created with a depth attachment flag. In raylib, `load_render_texture` creates a color + depth renderbuffer by default, but the depth is a renderbuffer, not a texture. For SSR, we need a **texture** we can sample. This may require a custom FBO via `rlgl` or a second depth-only render texture. If this is blocked, fall back to using the scene color's alpha channel as a depth proxy, or skip SSR and note the limitation.

**Fallback if depth texture is not sampleable:** Create a separate depth render pass — render the scene's depth into a `RenderTexture2D` using a depth-only shader (reuse `depth.fs`). This doubles as the shadow map approach but at screen resolution.

- [ ] **Step 4: Pass camera matrices and wetness from game.rs**

In `render()`, before `postfx.apply()`:

```rust
let view = Matrix::look_at(cam.position, cam.target, cam.up);
let proj = Matrix::perspective(cam.fovy.to_radians(), 1280.0 / 720.0, 0.01, 1000.0);
let view_proj = proj * view;
let inv_view_proj = Matrix::invert(view_proj);

// Wetness: higher at night for neon reflections
let h = total_hours.rem_euclid(24.0);
let wetness = if h < 6.0 || h > 20.0 { 0.8 } else if h < 8.0 || h > 18.0 { 0.4 } else { 0.0 };

self.postfx.set_ssr_data(proj, inv_view_proj, cam_pos, wetness);
```

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`
Expected: Build succeeds, all tests pass

- [ ] **Step 6: Runtime screenshot — verify reflections at night**

Capture a screenshot at night (0:00). Expected: road surfaces show faint reflections of neon lights and building windows above them. During day, no reflections (wetness = 0).

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: screen-space reflections — wet road neon reflections at night

Adds ssr.fs with 24-step screen-space ray-march for reflections.
Reconstructs world position from depth, estimates normals from depth
derivatives, and reflects composite color. Wetness uniform drives
road reflectivity — higher at night for neon street reflections.
Fresnel falloff at grazing angles."
```

---

## Self-Review Notes

### Spec coverage
- ✅ Bloom + tone mapping → Task 2 (bloom) + Task 3 (ACES in CRT)
- ✅ Screen-space reflections → Task 6
- ✅ Volumetric god rays → Task 5
- ✅ Chromatic aberration + scanlines → Task 3 (CRT)
- ✅ Starfield + skybox → Task 4
- ✅ PostFx struct → Task 1
- ✅ Implementation order matches spec

### Type consistency
- `PostFx::apply` signature: starts as `&self`, becomes `&mut self` when shaders are mutated. All tasks use `&mut self`.
- `begin_scene` returns a texture-mode draw handle — caller does `begin_mode3D` on it.
- Shader uniform location caching pattern is consistent across all tasks.

### Known risks
- **SSR depth texture:** raylib's `load_render_texture` creates a depth *renderbuffer*, not a *texture*. Sampling depth in SSR may require a custom depth render pass. Task 6 includes a fallback plan.
- **Borrow checker:** `begin_texture_mode` and `begin_shader_mode` are RAII guards with mutable borrows. The exact nesting may need adjustment during implementation.
- **Performance:** 6 fullscreen passes + SSR ray-march may need optimization (reduce step count, lower SSR resolution).
