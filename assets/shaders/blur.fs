#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;
uniform vec2 u_direction;  // texel step: (1/width, 0) or (0, 1/height)

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
