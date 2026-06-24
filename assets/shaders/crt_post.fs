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

    // Chromatic aberration
    float caStrength = 0.002;
    float r = texture(texture0, uv + dir * caStrength).r;
    float g = texture(texture0, uv).g;
    float b = texture(texture0, uv - dir * caStrength).b;
    vec3 color = vec3(r, g, b);

    // Scanlines
    float scanline = sin(uv.y * u_resolution.y * 3.14159) * 0.04;
    color -= scanline;

    // Vignette
    float vig = 1.0 - 0.3 * length(dir);
    color *= vig;

    // ACES tone mapping
    color = aces(color);

    // Film grain
    float grain = fract(sin(dot(uv, vec2(12.9898, 78.233)) + u_time) * 43758.5453);
    color += (grain - 0.5) * 0.02;

    finalColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
