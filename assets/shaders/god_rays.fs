#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;       // composite scene
uniform vec2 u_sunScreenPos;      // sun position in screen UV space (0..1)
uniform float u_intensity;        // 0..1

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

    vec3 result = base + accumulation * u_intensity * 0.3;
    finalColor = vec4(result, 1.0);
}
