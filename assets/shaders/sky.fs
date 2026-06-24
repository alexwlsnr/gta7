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
