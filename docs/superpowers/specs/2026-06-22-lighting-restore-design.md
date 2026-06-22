# Lighting Restore Design Spec

## Goal

Restore visible model lighting without reintroducing the black-geometry failure. This pass brings back directional sun/moon shading plus ambient fill for model-rendered geometry, while intentionally leaving fog and shadow darkening disabled in the final combine step.

## Current State

- The render pipeline still contains the full lighting system structure:
  - shadow map render target
  - depth pass before the main draw
  - per-frame lighting uniform updates
  - lit shader applied to model materials
- `assets/shaders/lighting.fs` is currently in a debug pass-through mode:
  - `baseColor = texture(texture0, fragTexCoord).rgb * fragColor.rgb`
  - output is just `baseColor`
- A fresh runtime capture confirmed the scene now renders again:
  - roads, cars, buildings, HUD, and player are visible
  - the remaining issue is visual flatness, not missing geometry

## Architecture

### Restore only the lighting math

Keep the current render pipeline intact and restore only the fragment shader's lighting combine:

1. Sample albedo color from the model material texture:
   - `texture(texture0, fragTexCoord)`
2. Multiply by raylib's diffuse tint passed from the vertex shader:
   - `fragColor.rgb`
3. Compute a normalized surface normal from `fragNormal`
4. Compute directional diffuse light from `u_lightDir` and `u_lightColor`
5. Add ambient fill from `u_ambientColor`
6. Output `ambient + diffuse`

This pass does **not** use:
- shadow darkening from `u_shadowMap`
- fog blending from `u_fogColor` / `u_fogDensity`

Those uniforms and the shadow pass remain wired in place, but the fragment shader ignores them until the lit base is visually correct again.

### Why this boundary

This is the narrowest safe restoration:

- It changes the smallest possible surface area: mostly `assets/shaders/lighting.fs`
- It preserves the existing shader/material contract that now successfully renders geometry
- It avoids another structural change to `src/game.rs` or `src/render/lighting.rs`
- It keeps future shadow/fog restoration cheap, because the plumbing remains intact

## Lighting Model For This Pass

### Base color

```glsl
vec3 baseColor = texture(texture0, fragTexCoord).rgb * fragColor.rgb;
```

### Normal handling

- Normalize `fragNormal` before lighting
- Guard against degenerate/zero normals so immediate-mode or malformed geometry cannot produce NaNs
- Fallback normal should be stable and boring rather than mathematically perfect

### Diffuse term

```glsl
float diff = max(dot(normal, lightDir), 0.0);
vec3 diffuse = u_lightColor * baseColor * diff;
```

### Ambient term

```glsl
vec3 ambient = u_ambientColor * baseColor;
```

### Final output

```glsl
vec3 lit = ambient + diffuse;
finalColor = vec4(lit, fragColor.a);
```

No fog mix. No shadow attenuation.

## Files

| File | Responsibility |
|---|---|
| `assets/shaders/lighting.fs` | Restore ambient + directional diffuse lighting and remove debug pass-through output |
| `assets/shaders/lighting.vs` | Keep current raylib-compatible model inputs (`mvp`, `matModel`, `colDiffuse`) unless runtime evidence requires a paired fix |
| `src/render/lighting.rs` | Keep current shader loading, uniform caching, day/night color feeding, and shadow/fog plumbing unchanged unless a tiny cleanup is required |
| `src/game.rs` | Keep current shadow-pass + main-pass orchestration unchanged for this restore |

## Constraints

- Must preserve the current working render state — no regression to black geometry
- Must keep HUD and pause menu visibility unchanged
- Must keep the day/night-driven light direction and color inputs already provided by `src/render/lighting.rs`
- Must not remove the existing shadow/depth pass scaffolding in this pass
- Must compile on the current GLSL 330 / raylib OpenGL 3.3 path

## Verification

### Build and tests

- `cargo build`
- `cargo test`

### Runtime check

Launch the game and capture a fresh frame. Success means:

- buildings, cars, and player model parts are visibly lit instead of flat debug-colored
- geometry colors remain correct
- HUD remains readable
- no black-object regression

## Non-Goals For This Pass

- Re-enabling shadow darkening
- Re-enabling fog blending
- Retuning ambient/fog/night thresholds
- Reworking the render architecture
- Adding lighting toggles or debug UI
