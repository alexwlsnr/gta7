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

uniform float u_windowGlow;

uniform vec3 u_light0_pos;
uniform vec3 u_light0_color;
uniform float u_light0_radius;

uniform vec3 u_light1_pos;
uniform vec3 u_light1_color;
uniform float u_light1_radius;

uniform vec3 u_light2_pos;
uniform vec3 u_light2_color;
uniform float u_light2_radius;

uniform vec3 u_light3_pos;
uniform vec3 u_light3_color;
uniform float u_light3_radius;

uniform vec3 u_light4_pos;
uniform vec3 u_light4_color;
uniform float u_light4_radius;

uniform vec3 u_light5_pos;
uniform vec3 u_light5_color;
uniform float u_light5_radius;

uniform int u_light_count;

out vec4 finalColor;

float compute_shadow() {
    if (fragLightSpacePos.w == 0.0) {
        return 1.0;
    }
    // Perform perspective/orthogonal projection divide
    vec3 projCoords = fragLightSpacePos.xyz / fragLightSpacePos.w;
    // Transform to [0,1] range
    projCoords = projCoords * 0.5 + 0.5;
    // Keep shadows inside light frustum boundaries
    if (projCoords.x < 0.0 || projCoords.x > 1.0 || 
        projCoords.y < 0.0 || projCoords.y > 1.0 || 
        projCoords.z > 1.0) {
        return 1.0; // fully lit
    }
    float closestDepth = texture(u_shadowMap, projCoords.xy).r;
    float currentDepth = projCoords.z;
    // Shadow bias to prevent acne.
    float bias = 0.005;
    return currentDepth - bias < closestDepth ? 1.0 : 0.4;
}

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

    // Diffuse term with shadow attenuation
    float diff = max(dot(normal, lightDir), 0.0);
    float shadow = compute_shadow();
    vec3 diffuse = u_lightColor * baseColor * diff * shadow;

    // Ambient term
    vec3 ambient = u_ambientColor * baseColor;

    // Point lights (up to 6)
    vec3 pointLightDiffuse = vec3(0.0);
    if (u_light_count >= 1) {
        vec3 lToPos = u_light0_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light0_radius) {
            float att = 1.0 - (dist / u_light0_radius);
            pointLightDiffuse += u_light0_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }
    if (u_light_count >= 2) {
        vec3 lToPos = u_light1_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light1_radius) {
            float att = 1.0 - (dist / u_light1_radius);
            pointLightDiffuse += u_light1_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }
    if (u_light_count >= 3) {
        vec3 lToPos = u_light2_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light2_radius) {
            float att = 1.0 - (dist / u_light2_radius);
            pointLightDiffuse += u_light2_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }
    if (u_light_count >= 4) {
        vec3 lToPos = u_light3_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light3_radius) {
            float att = 1.0 - (dist / u_light3_radius);
            pointLightDiffuse += u_light3_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }
    if (u_light_count >= 5) {
        vec3 lToPos = u_light4_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light4_radius) {
            float att = 1.0 - (dist / u_light4_radius);
            pointLightDiffuse += u_light4_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }
    if (u_light_count >= 6) {
        vec3 lToPos = u_light5_pos - fragWorldPos;
        float dist = length(lToPos);
        if (dist < u_light5_radius) {
            float att = 1.0 - (dist / u_light5_radius);
            pointLightDiffuse += u_light5_color * baseColor * max(dot(normal, normalize(lToPos)), 0.0) * att * att;
        }
    }

    // Final output combining ambient + diffuse + point light diffuse
    vec3 lit = ambient + diffuse + pointLightDiffuse;

    // Building window emissive glow at night
    // Yellow color in texture corresponds to RGB approx: r > 0.9, g > 0.8, b < 0.75.
    bool isWindow = (texColor.r > 0.9 && texColor.g > 0.8 && texColor.b < 0.75);
    if (isWindow) {
        vec3 windowColor = vec3(1.0, 0.9, 0.6);
        lit = mix(lit, windowColor, u_windowGlow);
    }

    // Exponential fog based on camera distance
    float dist = length(u_cameraPos - fragWorldPos);
    float fogFactor = 1.0 - exp(-u_fogDensity * dist);
    fogFactor = clamp(fogFactor, 0.0, 1.0);

    vec3 final = mix(lit, u_fogColor, fogFactor);
    finalColor = vec4(final, fragColor.a * texColor.a);
}

