#version 330 core

layout(location = 0) out vec4 fragColor;

void main() {
    // Output linearized depth in red channel (enough for shadow comparison).
    float depth = gl_FragCoord.z;
    fragColor = vec4(depth, 0.0, 0.0, 1.0);
}
