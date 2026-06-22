#version 330 core

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

// Automatically set by raylib.
uniform mat4 mvp;
uniform mat4 modelMatrix;
uniform vec4 colDiffuse;

// Custom uniforms set from Rust.
uniform mat4 lightSpaceMatrix;

out vec3 fragWorldPos;
out vec2 fragTexCoord;
out vec3 fragNormal;
out vec4 fragLightSpacePos;
out vec4 fragColor;

void main() {
    vec4 worldPos = modelMatrix * vec4(vertexPosition, 1.0);
    fragWorldPos = worldPos.xyz;
    fragTexCoord = vertexTexCoord;
    // Guard against zero normals from immediate-mode draws.
    vec3 n = mat3(modelMatrix) * vertexNormal;
    if (length(n) < 0.001) {
        n = vec3(0.0, 1.0, 0.0);
    }
    fragNormal = n;
    fragLightSpacePos = lightSpaceMatrix * worldPos;
    fragColor = colDiffuse;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
