#version 330 core

in vec2 fragTexCoord;
uniform sampler2D texture0;       // scene
uniform sampler2D texture1;       // bloom
uniform float u_bloomStrength;    // 1.2

out vec4 finalColor;

void main() {
    vec3 scene = texture(texture0, fragTexCoord).rgb;
    vec3 bloom = texture(texture1, fragTexCoord).rgb;
    vec3 result = scene + bloom * u_bloomStrength;
    // Reinhard tone mapping to prevent blowout
    result = result / (1.0 + result);
    finalColor = vec4(result, 1.0);
}
