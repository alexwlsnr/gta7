#version 330 core
//
// Screen-Space Reflections (simplified first pass).
//
// Full SSR reconstructs world position from a depth texture, marches a
// reflection ray in world space, and resolves against scene depth. raylib's
// `load_render_texture` creates a depth *renderbuffer* (not a texture), so
// depth cannot be sampled in a shader. This simplified version uses
// color-based heuristics instead:
//
//   1. Sky check  — skip pixels that look like sky (very bright, low
//      saturation). Sky pixels are above the horizon and don't reflect.
//   2. Normal    — estimate a screen-space surface normal from a Sobel-like
//      luminance kernel. For ground/road surfaces the vertical gradient
//      points from dark (below) to bright (above), giving a strong
//      upward-facing component. Surfaces that aren't upward-facing
//      (walls, sky, building tops) get a low ground factor and are skipped.
//   3. March     — for upward-facing ground pixels, walk upward in screen
//      space for 24 steps and accumulate colors with a distance-based
//      falloff. The accumulation approximates a wet-road mirror: a road
//      pixel near the horizon "sees" the buildings/sky directly above it;
//      pixels further from the horizon average over a longer stretch.
//
// Result is mixed back into the base color with a reflectivity factor
// driven by `u_wetness` (0 = no reflection, 0.8 = full nighttime).
//
// Reserved for a future depth-texture upgrade: `u_proj`, `u_invViewProj`,
// `u_cameraPos` are uploaded by PostFx and could be used for proper
// world-space ray marching against a depth render target.
//

in vec2 fragTexCoord;

uniform sampler2D texture0;       // composite color (post-bloom)
uniform float u_wetness;          // 0..0.8 — wetness scalar
uniform vec2 u_resolution;        // screen dimensions for texel sampling

out vec4 finalColor;

float luminance(vec3 c) {
    return dot(c, vec3(0.299, 0.587, 0.114));
}

float saturation(vec3 c) {
    float mx = max(c.r, max(c.g, c.b));
    float mn = min(c.r, min(c.g, c.b));
    return (mx - mn) / max(mx, 0.001);
}

void main() {
    vec3 base = texture(texture0, fragTexCoord).rgb;

    // Daytime: skip the entire pass.
    if (u_wetness < 0.01) {
        finalColor = vec4(base, 1.0);
        return;
    }

    vec2 texel = 1.0 / u_resolution;

    // Sky heuristic: very bright + low saturation = sky dome. No reflection.
    float baseLum = luminance(base);
    float baseSat = saturation(base);
    if (baseLum > 0.6 && baseSat < 0.2) {
        finalColor = vec4(base, 1.0);
        return;
    }

    // Sobel-like luminance kernel for a screen-space normal estimate.
    float lL = luminance(texture(texture0, fragTexCoord - vec2(texel.x, 0.0)).rgb);
    float lR = luminance(texture(texture0, fragTexCoord + vec2(texel.x, 0.0)).rgb);
    float lU = luminance(texture(texture0, fragTexCoord - vec2(0.0, texel.y)).rgb);
    float lD = luminance(texture(texture0, fragTexCoord + vec2(0.0, texel.y)).rgb);

    // Screen-space gradient (y points down in UV space).
    float gx = lR - lL;
    float gy = lD - lU;

    // Normal in screen space. For a horizontal ground surface, gy is strongly
    // positive (sky is brighter above than the road below), so the y-axis
    // component is large. Walls and sky have weak vertical gradient.
    vec3 normal = normalize(vec3(-gx * 4.0, 1.0, -gy * 4.0));

    // Only reflect upward-facing surfaces (roads, sidewalks, etc.).
    float groundFactor = clamp(normal.y, 0.0, 1.0);
    if (groundFactor < 0.3) {
        finalColor = vec4(base, 1.0);
        return;
    }

    // 24-step screen-space ray-march. For a ground pixel the reflection of
    // the sky/buildings appears at the screen position mirrored across the
    // horizon — walk upward in screen Y, weighted by distance.
    vec3 reflection = vec3(0.0);
    float totalWeight = 0.0;
    bool found = false;

    const int STEPS = 24;
    for (int i = 1; i <= STEPS; i++) {
        float t = float(i) / float(STEPS);
        // Reach a maximum of 25% of screen height above the current pixel.
        vec2 sUV = vec2(fragTexCoord.x, fragTexCoord.y - t * 0.25);
        if (sUV.y < 0.0) break;

        vec3 sColor = texture(texture0, sUV).rgb;
        float sLum = luminance(sColor);
        float sSat = saturation(sColor);

        bool sampleIsSky = (sLum > 0.55 && sSat < 0.25);
        float weight = 1.0 - t * 0.5;
        if (sampleIsSky) {
            // Sky contributes dimmer — we want the neon building windows
            // and lit signs to dominate the reflection.
            weight *= 0.35;
        } else {
            // Lit structures (windows, signs) get a slight boost.
            weight *= 1.1;
        }

        reflection += sColor * weight;
        totalWeight += weight;
        found = true;
    }

    if (!found || totalWeight < 0.001) {
        finalColor = vec4(base, 1.0);
        return;
    }

    reflection /= totalWeight;

    // Final reflectivity: ground alignment * wetness, capped to prevent
    // washing out the base color.
    float reflectivity = clamp(groundFactor * u_wetness * 0.7, 0.0, 0.8);

    vec3 result = mix(base, reflection, reflectivity);
    finalColor = vec4(result, 1.0);
}
