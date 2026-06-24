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

uniform float u_metallic;
uniform float u_roughness;
uniform float u_specular;

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
    vec3 projCoords = fragLightSpacePos.xyz / fragLightSpacePos.w;
    projCoords = projCoords * 0.5 + 0.5;
    if (projCoords.x < 0.0 || projCoords.x > 1.0 || 
        projCoords.y < 0.0 || projCoords.y > 1.0 || 
        projCoords.z > 1.0) {
        return 1.0;
    }
    float currentDepth = projCoords.z;
    float bias = 0.003;
    float shadow = 0.0;
    vec2 texelSize = 1.0 / textureSize(u_shadowMap, 0);
    for (int x = -1; x <= 1; x++) {
        for (int y = -1; y <= 1; y++) {
            float pcfDepth = texture(u_shadowMap, projCoords.xy + vec2(x, y) * texelSize).r;
            shadow += currentDepth - bias < pcfDepth ? 1.0 : 0.35;
        }
    }
    shadow /= 9.0;
    return shadow;
}

void accumulatePointLight(
    vec3 p_pos, vec3 p_color, float p_radius,
    vec3 normal, vec3 fragWorldPos, vec3 viewDir,
    vec3 baseColor, vec3 specColor, float activeSpecular, float shininess,
    inout vec3 diffuseAccum, inout vec3 specularAccum
) {
    vec3 lToPos = p_pos - fragWorldPos;
    float dist = length(lToPos);
    if (dist < p_radius) {
        float att = 1.0 - (dist / p_radius);
        vec3 lDir = lToPos / dist;
        
        // Diffuse
        diffuseAccum += p_color * baseColor * max(dot(normal, lDir), 0.0) * att * att;
        
        // Specular
        vec3 halfDirPt = normalize(lDir + viewDir);
        float specPt = pow(max(dot(normal, halfDirPt), 0.0), shininess);
        specularAccum += p_color * specColor * activeSpecular * specPt * att * att;
    }
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

    // Building window emissive glow at night (tagged with alpha = 254/255)
    bool isWindow = (texColor.a > 0.992 && texColor.a < 0.998);
    
    // Specular and metallic properties (override for building windows to make them glossy)
    float activeMetallic = isWindow ? 0.0 : u_metallic;
    float activeRoughness = isWindow ? 0.05 : u_roughness;
    float activeSpecular = isWindow ? 1.0 : u_specular;

    float shininess = 8.0 + (1.0 - activeRoughness) * 120.0;
    vec3 specColor = mix(vec3(1.0), baseColor, activeMetallic);

    // View direction
    vec3 viewDir = vec3(0.0, 0.0, 1.0);
    vec3 toCam = u_cameraPos - fragWorldPos;
    float toCamLen = length(toCam);
    if (toCamLen > 0.0001) {
        viewDir = toCam / toCamLen;
    }

    // Diffuse term with shadow attenuation
    float diff = max(dot(normal, lightDir), 0.0);
    float shadow = compute_shadow();
    vec3 diffuse = u_lightColor * baseColor * diff * shadow;

    // Specular term for directional light
    vec3 halfDir = normalize(lightDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), shininess);
    vec3 specular = u_lightColor * specColor * activeSpecular * spec * shadow;

    // Ambient term
    vec3 ambient = u_ambientColor * baseColor;

    // Point lights (up to 6)
    vec3 pointLightDiffuse = vec3(0.0);
    vec3 pointLightSpecular = vec3(0.0);
    if (u_light_count >= 1) {
        accumulatePointLight(u_light0_pos, u_light0_color, u_light0_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }
    if (u_light_count >= 2) {
        accumulatePointLight(u_light1_pos, u_light1_color, u_light1_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }
    if (u_light_count >= 3) {
        accumulatePointLight(u_light2_pos, u_light2_color, u_light2_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }
    if (u_light_count >= 4) {
        accumulatePointLight(u_light3_pos, u_light3_color, u_light3_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }
    if (u_light_count >= 5) {
        accumulatePointLight(u_light4_pos, u_light4_color, u_light4_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }
    if (u_light_count >= 6) {
        accumulatePointLight(u_light5_pos, u_light5_color, u_light5_radius, normal, fragWorldPos, viewDir, baseColor, specColor, activeSpecular, shininess, pointLightDiffuse, pointLightSpecular);
    }

    // Final output combining ambient + diffuse + specular + point light diffuse + point light specular
    vec3 lit = ambient + diffuse + specular + pointLightDiffuse + pointLightSpecular;

    // Fresnel rim lighting — highlights edges of geometry facing away from camera
    float fresnel = pow(1.0 - max(dot(normal, viewDir), 0.0), 3.0);
    vec3 rimColor = u_lightColor * 0.4 * fresnel * (1.0 - activeRoughness);
    lit += rimColor;

    // Metallic environment reflection (sky-based fake cubemap)
    if (activeMetallic > 0.1) {
        vec3 reflDir = reflect(-viewDir, normal);
        float skyGrad = reflDir.y * 0.5 + 0.5;
        vec3 envColor = mix(vec3(0.15, 0.12, 0.10), u_fogColor, skyGrad);
        lit = mix(lit, envColor * baseColor * 2.0, activeMetallic * 0.35 * (1.0 - activeRoughness));
    }

    // Building window emissive glow at night
    if (isWindow) {
        vec3 windowColor = texColor.rgb * 2.0;
        lit = mix(lit, windowColor, u_windowGlow);
    }

    // Exponential fog based on camera distance
    float dist = length(u_cameraPos - fragWorldPos);
    float fogFactor = 1.0 - exp(-u_fogDensity * dist);
    fogFactor = clamp(fogFactor, 0.0, 1.0);

    vec3 final = mix(lit, u_fogColor, fogFactor);
    finalColor = vec4(final, fragColor.a * texColor.a);
}

