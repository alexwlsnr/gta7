#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;
uniform float u_threshold;  // 0.7
uniform float u_softKnee;   // 0.3

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
