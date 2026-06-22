# Lighting, Shadows & Fog Design Spec

## Goal

Transform the flat-shaded rendering into a next-gen look using directional lighting, real-time shadow mapping, and distance fog — all via custom GLSL shaders within raylib.

## Architecture

### Two-pass rendering pipeline

```
Pass 1: Shadow Map
  - Orthographic camera positioned at sun, aimed at player
  - Render buildings + vehicles + character bodies (shadow casters) as depth only
  - Output: RenderTexture2D (1024×1024 depth texture)

Pass 2: Main Scene (to screen)
  - Custom lighting shader active via begin_shader_mode
  - Shader computes: directional light + ambient + shadow factor + fog
  - All 3D draw calls (models, cubes, cylinders, planes) go through shader
  - Shadow map sampled via texture uniform

Pass 3: HUD (unchanged)
  - 2D overlay drawn after shader mode ends
```

### Lighting model

- **Directional light (sun):** direction vector derived from game time. Sun arcs east→west over 24h cycle. Light color shifts: warm orange at dawn/dusk (6.5h, 18.5h), white at noon (13h), dim blue moonlight at night (0h).
- **Ambient light:** derived from sky bottom color (already computed by `sky_colors_for_hour`). Fills shadowed areas so they're not pure black.
- **Fog:** exponential fog based on view distance. Fog color = sky bottom color. Density increases at night for atmosphere. Near objects clear, distant buildings fade into sky.

### Shader uniforms (updated per frame)

| Uniform | Type | Description |
|---|---|---|
| `u_lightDir` | vec3 | Sun direction (normalized) |
| `u_lightColor` | vec3 | Sun color (RGB, 0..1) |
| `u_ambientColor` | vec3 | Ambient/sky color (RGB, 0..1) |
| `u_fogColor` | vec3 | Fog color (= sky bottom) |
| `u_fogDensity` | float | Exponential fog density |
| `u_shadowMap` | sampler2D | Shadow map texture from Pass 1 |
| `u_lightSpaceMatrix` | mat4 | World→light clip space transform |
| `u_cameraPos` | vec3 | Camera world position (for fog distance) |

### Shadow map camera

- Orthographic projection (directional light = parallel rays)
- Follows player position so shadows are always centered around the action
- View volume: ~120×120 world units (covers visible area)
- Near/far: 1.0 to 300.0 (captures building heights)
- Shadow map resolution: 1024×1024

### Files

| File | Responsibility |
|---|---|
| `assets/shaders/lighting.vs` | Vertex shader: pass world pos, normal, view pos to fragment |
| `assets/shaders/lighting.fs` | Fragment shader: directional light + ambient + shadow + fog |
| `assets/shaders/depth.fs` | Fragment shader for shadow pass: output linear depth |
| `assets/shaders/depth.vs` | Vertex shader for shadow pass: minimal, output depth only |
| `src/render/lighting.rs` | LightingSystem struct: manages shaders, shadow map, uniforms, sun position |
| `src/render/models.rs` | Modified: use lighting shader for all 3D draws |
| `src/game.rs` | Modified: orchestrate shadow pass + lit pass in render() |
| `src/config.rs` | Modified: sun position computation from game time |

## Constraints

- Must work on OpenGL 3.3 (raylib's default backend)
- Shadow map capped at 1024×1024 for performance
- Only buildings, vehicles, and characters cast shadows (roads/sidewalks don't)
- Fog must not affect HUD or pause menu (2D overlay)
- Day/night cycle drives light direction and color — no separate light config
- Shaders loaded from files in `assets/shaders/` (not inline strings) for maintainability
