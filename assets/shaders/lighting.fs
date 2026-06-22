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

float compute_shadow() {
    vec3 projCoords = fragLightSpacePos.xyz / fragLightSpacePos.w;
    projCoords = projCoords * 0.5 + 0.5;
    if (projCoords.x < 0.0 || projCoords.x > 1.0 ||
        projCoords.y < 0.0 || projCoords.y > 1.0 ||
        projCoords.z > 1.0) {
        return 1.0;
    }
    float closestDepth = texture(u_shadowMap, projCoords.xy).r;
    float currentDepth = projCoords.z;
    float bias = 0.01;
    return currentDepth - bias < closestDepth ? 1.0 : 0.5;
}

void main() {
    vec4 texColor = texture(texture0, fragTexCoord);
    vec3 baseColor = texColor.rgb * fragColor.rgb;

    // Simple diffuse + ambient. No fog, no shadow first.
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(-u_lightDir);

    float diff = max(dot(normal, lightDir), 0.0);
    float shadow = compute_shadow();

    vec3 ambient = u_ambientColor * baseColor;
    vec3 diffuse = u_lightColor * baseColor * diff * shadow;
    vec3 lit = ambient + diffuse;

    // Fog
    float dist = length(u_cameraPos - fragWorldPos);
    float fogFactor = 1.0 - exp(-u_fogDensity * dist);
    fogFactor = clamp(fogFactor, 0.0, 1.0);
    vec3 final = mix(lit, u_fogColor, fogFactor);

    finalColor = vec4(final, fragColor.a);
}
