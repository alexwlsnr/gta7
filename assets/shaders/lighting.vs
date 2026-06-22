#version 330 core

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

// Automatically set by raylib.
uniform mat4 mvp;
uniform mat4 matModel;
uniform vec4 colDiffuse;

// Custom uniforms set from Rust.
uniform mat4 lightSpaceMatrix;

out vec3 fragWorldPos;
out vec2 fragTexCoord;
out vec3 fragNormal;
out vec4 fragLightSpacePos;
out vec4 fragColor;

void main() {
    vec4 worldPos = matModel * vec4(vertexPosition, 1.0);
    fragWorldPos = worldPos.xyz;
    fragTexCoord = vertexTexCoord;
    fragNormal = mat3(matModel) * vertexNormal;
    fragLightSpacePos = lightSpaceMatrix * worldPos;
    fragColor = colDiffuse;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
