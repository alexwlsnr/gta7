#version 330 core

in vec3 fragWorldPos;
in vec2 fragTexCoord;
in vec3 fragNormal;
in vec4 fragLightSpacePos;
in vec4 fragColor;

uniform vec3 u_lightDir;
uniform vec3 u_lightColor;
uniform vec3 u_ambientColor;
uniform vec3 u_fogColor;
uniform float u_fogDensity;
uniform vec3 u_cameraPos;
uniform sampler2D u_shadowMap;
uniform sampler2D texture0;

out vec4 finalColor;

void main() {
    // DEBUG: Output solid red to verify shader is active.
    // If we see red, the shader works and the issue is in the lighting math.
    // If we see black, the shader isn't loading or matModel is wrong.
    finalColor = vec4(1.0, 0.0, 0.0, 1.0);
}
