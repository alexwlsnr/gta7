#version 330 core

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 model;
uniform mat4 lightSpaceMatrix;

out vec3 fragWorldPos;
out vec2 fragTexCoord;
out vec3 fragNormal;
out vec4 fragLightSpacePos;

void main() {
    vec4 worldPos = model * vec4(vertexPosition, 1.0);
    fragWorldPos = worldPos.xyz;
    fragTexCoord = vertexTexCoord;
    fragNormal = mat3(model) * vertexNormal;
    fragLightSpacePos = lightSpaceMatrix * worldPos;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
