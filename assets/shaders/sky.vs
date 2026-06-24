#version 330 core

in vec3 vertexPosition;

uniform mat4 mvp;
uniform mat4 matModel;

out vec3 fragWorldDir;

void main() {
    fragWorldDir = normalize(vertexPosition);
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
