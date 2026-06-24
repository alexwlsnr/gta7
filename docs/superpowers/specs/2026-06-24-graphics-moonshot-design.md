# Graphics Moonshot: Full Post-Processing Pipeline Design Spec

## Goal

Transform the vaporwave neon city from a direct-to-screen render into a multi-pass post-processing pipeline with bloom, tone mapping, screen-space reflections, volumetric god rays, a CRT/vaporwave aesthetic filter, and a procedural starfield + sky dome.

## Current State

The render pipeline renders 3D geometry directly to the default framebuffer:

```
Pass 0: Shadow map (2048×2048 depth texture, PCF 3×3)
Pass 1: 3D scene → screen (custom lit shader on model materials)
Pass 2: HUD (2D overlay on screen)
```

The lit shader already supports:
- Ambient + directional diffuse + specular
- 6 point lights (headlights, taillights, streetlights, sirens)
- Fresnel rim lighting
- Metallic environment reflection (sky-based fake cubemap)
- Building window emissive glow (alpha-tagged)
- Exponential distance fog
- Shadow attenuation

The sky is drawn with ~360 CPU `draw_rectangle` calls per frame as gradient bands.

No post-processing exists. Everything goes straight to the screen.

## Architecture

### Multi-pass FBO pipeline

Render the 3D scene to an offscreen texture, then chain fullscreen shader passes before outputting to screen:

```
Pass 0: Shadow map (existing, unchanged)
Pass 1: Scene → scene_fbo (1280×720, RGBA + depth)
  - Sky dome mesh with gradient shader (replaces CPU sky bands)
  - Starfield sky dome (procedural texture, alpha-faded by time of day)
  - All 3D world geometry + entities (existing draw calls, redirected to FBO)
Pass 2: Bright extract → bright_fbo (half-res 640×360)
  - Threshold luminance > 0.7, soft knee
Pass 3: Blur H → blur_fbo[0] (half-res, 9-tap Gaussian, horizontal)
Pass 4: Blur V → blur_fbo[1] (half-res, 9-tap Gaussian, vertical)
  - Repeat passes 3-4 for a second blur iteration (wider glow)
Pass 5: Bloom composite → composite_fbo (full-res)
  - scene + bloom * u_bloomStrength (additive, strength ~1.2)
  - Soft tone curve: final / (1.0 + final * 0.3)
Pass 6: SSR → ssr_fbo (full-res)
  - Ray-march reflected view direction in screen space (16-32 steps)
  - Sample composite color at hit position
  - Fresnel falloff: grazing angles reflect more
  - Only reflect metallic surfaces (u_metallic > 0.1) or wet roads (u_wetness > 0)
  - Fallback: reflect sky/fog color on miss
Pass 7: God rays → composite_fbo (additive)
  - Radial blur from sun screen-space position
  - 32 samples, decrementing weight
  - Intensity driven by sun elevation — zero at noon, peak at dawn/dusk
Pass 8: CRT post → screen
  - Chromatic aberration: RGB channels sampled at offset UVs
  - Scanlines: sin-based brightness modulation
  - Vignette: radial darkening at edges
  - ACES filmic tone mapping: maps HDR-ish values to [0,1]
  - Subtle film grain: hash-based noise
Pass 9: HUD → screen (2D overlay, unchanged, drawn after post-processing)
```

### PostFx struct

New struct in `src/render/postfx.rs`:

```rust
pub struct PostFx {
    scene_fbo: RenderTexture2D,       // 1280×720, RGBA + depth
    bright_fbo: RenderTexture2D,      // 640×360, RGBA
    blur_fbo: [RenderTexture2D; 2],   // 640×360, RGBA (ping-pong)
    composite_fbo: RenderTexture2D,    // 1280×720, RGBA
    ssr_fbo: RenderTexture2D,          // 1280×720, RGBA

    bright_shader: Shader,
    blur_shader: Shader,
    bloom_shader: Shader,
    ssr_shader: Shader,
    god_rays_shader: Shader,
    crt_shader: Shader,
    sky_shader: Shader,

    // Cached uniform locations
    // ...per shader
}
```

- `begin_scene()` → returns a `RaylibDraw3D` handle for drawing 3D into scene_fbo
- `apply()` → runs all post-processing passes, outputs to screen
- `resize()` → recreates all FBOs if screen size changes

### Render texture sizes

| FBO | Size | Rationale |
|---|---|---|
| scene_fbo | 1280×720 | Full scene resolution with depth |
| bright_fbo | 640×360 | Half-res for bloom — blur is expensive, lower res is imperceptible |
| blur_fbo[0..1] | 640×360 | Ping-pong blur at half-res |
| composite_fbo | 1280×720 | Full-res composite of scene + bloom |
| ssr_fbo | 1280×720 | Full-res for reflection quality |

## Effect Details

### Bloom

**Bright extract** (`assets/shaders/bright_extract.fs`):
- Luminance: `dot(rgb, vec3(0.2126, 0.7152, 0.0722))`
- Soft threshold: if luminance > 0.7, output `rgb * (luminance - 0.7) / 0.3`
- Otherwise output black
- Isolates neon lights, window glow, headlights, explosions, muzzle flashes

**Blur** (`assets/shaders/blur.fs`):
- Separable 9-tap Gaussian blur
- Uniform `u_direction` (vec2): `(1/width, 0)` for horizontal, `(0, 1/height)` for vertical
- Two iterations of H+V for wider, softer glow

**Bloom composite** (`assets/shaders/bloom_composite.fs`):
- `final = scene + bloom * u_bloomStrength` (additive, strength ~1.2)
- Soft tone curve: `final = final / (1.0 + final * 0.3)` to prevent blowout

### Tone Mapping

ACES filmic approximation, applied in the CRT shader as the final step:
- `a = 2.51`, `b = 0.03`, `c = 2.43`, `d = 0.59`, `e = 0.14`
- `mapped = clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0)`
- All effects operate in pre-tone-map space; ACES maps to display range at the end

### Screen-Space Reflections

**SSR shader** (`assets/shaders/ssr.fs`):
- Requires scene depth texture from scene_fbo
- Reconstruct world position from screen UV + depth
- Compute reflected view direction: `reflect(-viewDir, normal)`
- Project reflection direction to screen space
- Ray-march 16-32 steps along screen-space direction, sampling depth
- When ray hits geometry (depth match within threshold ~0.01), sample composite color
- Fresnel falloff: `pow(1.0 - max(dot(normal, viewDir), 0.0), 3.0)`
- Only reflect surfaces with `u_metallic > 0.1` or tagged as wet roads
- Blend: `final = mix(composite, reflection, fresnel * reflectivity)`
- Miss fallback: reflect sky/fog color

**Wet roads:** `u_wetness` uniform (0..1), driven by time of day — wetter at night for neon reflections in streets.

### Volumetric God Rays

**God rays shader** (`assets/shaders/god_rays.fs`):
- Compute sun screen-space position: project `sun_position(hour, player_pos)` via camera matrix
- For each pixel, sample along direction toward sun screen pos
- 32 samples, linearly decrementing weight
- Creates radial light shaft effect
- Additive blend: `composite += godRays * u_godRayIntensity`
- Intensity from sun elevation: zero at noon, peak at dawn/dusk, zero at night
- Driven by `sun_direction(hour).y` — higher elevation = less god ray intensity

### CRT / Vaporwave Post Filter

**CRT shader** (`assets/shaders/crt_post.fs`):
- **Chromatic aberration:** sample R at `uv + 0.002 * (uv - 0.5)`, B at `uv - 0.002 * (uv - 0.5)`, G at `uv`
- **Scanlines:** `sin(uv.y * screenHeight * PI) * 0.04` subtracted from final brightness
- **Vignette:** `1.0 - 0.3 * length(uv - 0.5)` multiplied into final
- **ACES tone mapping:** applied after all other effects, maps to [0,1]
- **Film grain:** `fract(sin(dot(uv, vec2(12.9898, 78.233)) + u_time) * 43758.5453) * 0.02` added

### Starfield + Sky Dome

**Starfield** (procedural, in `src/render/models.rs`):
- Generate 512×512 star texture at load time
- Random pixels with brightness 0.5-1.0, density ~0.5%
- A few "neon" stars with pink/cyan tint
- Apply to a large sphere mesh (sky dome) rendered inside-out
- Alpha fade by time of day: visible at night, invisible during day

**Sky dome** (`assets/shaders/sky.vs` + `assets/shaders/sky.fs`):
- Large sphere mesh, rendered inside-out (front face culling)
- Fragment shader computes sky color from view direction Y component
- Uses time-of-day uniforms (sky_top, sky_bottom colors) to gradient
- Replaces the current ~360 CPU `draw_rectangle` calls per frame
- Single mesh draw call instead

## Files

### New files

| File | Responsibility |
|---|---|
| `src/render/postfx.rs` | PostFx struct: owns all FBOs, shaders, pass orchestration |
| `assets/shaders/bright_extract.fs` | Threshold bright pixels for bloom |
| `assets/shaders/blur.fs` | Separable Gaussian blur |
| `assets/shaders/bloom_composite.fs` | Additive bloom + soft tone curve |
| `assets/shaders/ssr.fs` | Screen-space reflections via depth ray-march |
| `assets/shaders/god_rays.fs` | Radial blur god rays from sun position |
| `assets/shaders/crt_post.fs` | Chromatic aberration + scanlines + vignette + ACES + grain |
| `assets/shaders/sky.fs` | Sky dome gradient shader |
| `assets/shaders/sky.vs` | Sky dome vertex shader |

### Modified files

| File | Changes |
|---|---|
| `src/game.rs` | render() redirects 3D to scene_fbo via PostFx, removes CPU sky bands, calls postfx.apply() before HUD |
| `src/render/mod.rs` | Add `pub mod postfx;` |
| `src/render/models.rs` | Add starfield texture + sky dome model generation in Assets |
| `src/render/lighting.rs` | Expose scene depth texture reference for SSR |
| `src/config.rs` | Add god ray intensity computation from sun elevation |

## Constraints

- Must work on OpenGL 3.3 (raylib's default backend, GLSL 330)
- Must maintain 60fps at 1280×720 on the target hardware (RTX 5080)
- Must not regress to black geometry or break HUD/pause menu visibility
- Must keep all 28 existing tests passing
- Each post-processing pass must be individually verifiable before moving to the next
- Shaders loaded from files in `assets/shaders/` (not inline strings)
- FBOs recreated on window resize

## Verification

### Per-pass verification (incremental)

After each pass is added:
1. `cargo build`
2. `cargo test` — all 28 tests must pass
3. Runtime screenshot — verify visual output of the new pass
4. Check no regression: HUD readable, pause menu functional, no black geometry

### Final verification

1. Full `cargo build` + `cargo test`
2. Runtime screenshot — all effects visible: bloom glow, reflections in roads, god rays at dawn/dusk, CRT scanlines/vignette, starfield at night
3. Performance: 60fps at 1280×720

## Implementation Order

Build incrementally, one pass at a time, verifying each:

1. **PostFx scaffold** — scene_fbo + blit to screen (no visual change, proves FBO pipeline works)
2. **Bloom** — bright extract + blur + composite (biggest visual impact)
3. **CRT post** — chromatic aberration + scanlines + vignette + ACES tone map
4. **Starfield + sky dome** — replace CPU sky bands with sky dome mesh + starfield
5. **God rays** — radial blur from sun position
6. **SSR** — screen-space reflections (most complex, last)

## Non-Goals

- Deferred rendering (forward renderer stays)
- Real cubemap environment mapping (fake sky-based reflection stays)
- Temporal anti-aliasing (TAA)
- Motion blur
- Depth of field
- Weather system (rain/snow)
- Texture streaming or LOD system
- Mesh loading from files (all geometry stays procedural)
