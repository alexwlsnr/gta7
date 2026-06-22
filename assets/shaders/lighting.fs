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
    vec4 texColor = texture(texture0, fragTexCoord);
    vec3 baseColor = texColor.rgb * fragColor.rgb;

    // Normalize vectors and guard against degenerate values
    vec3 normal = vec3(0.0, 1.0, 0.0);
    float normalLen = length(fragNormal);
    if (normalLen > 0.0001) {
        normal = fragNormal / normalLen;
    }

    vec3 lightDir = vec3(0.0, 1.0, 0.0);
    float lightDirLen = length(u_lightDir);
    if (lightDirLen > 0.0001) {
        lightDir = -u_lightDir / lightDirLen;
    }

    // Diffuse term
    float diff = max(dot(normal, lightDir), 0.0);
    vec3 diffuse = u_lightColor * baseColor * diff;

    // Ambient term
    vec3 ambient = u_ambientColor * baseColor;

    // Final output combining ambient + diffuse
    vec3 lit = ambient + diffuse;
    finalColor = vec4(lit, fragColor.a * texColor.a);
}
