shader_type spatial;
render_mode diffuse_burley, specular_schlick_ggx;
varying vec3 world_position;
varying vec3 world_normal;

float hash3(vec3 p) { return fract(sin(dot(p, vec3(127.1,311.7,74.7))) * 43758.5453); }
void vertex() {
    world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz;
    world_normal = normalize(MODEL_NORMAL_MATRIX * NORMAL);
}
void fragment() {
    vec2 pixel = floor(UV * 16.0);
    vec3 seed = floor(world_position - world_normal * 0.01);
    float noise = hash3(vec3(pixel,0.0) + seed * 19.0);
    float detail = 0.91 + noise * 0.16;
    int pattern = int(round(UV2.x));
    if (pattern == 0) { detail = 1.0; }
    if (pattern == 1) { detail = 0.88 + hash3(vec3(floor(UV.x*8.0),floor(UV.y*2.0),seed.x+seed.z))*0.2; }
    if (pattern == 2) {
        float row = floor(UV.y * 2.0);
        vec2 p = fract(UV * vec2(2.0) + vec2(row*0.5,0.0));
        if (p.x < 0.045 || p.y < 0.055) { detail *= 0.74; }
    }
    if (pattern == 4) { detail = 0.75 + noise * 0.35; }
    if (pattern == 5 || pattern == 6) {
        detail = 0.87 + noise * 0.18;
        // Planks, iron corner bands and lid seam; +Z is the latch face.
        if (mod(pixel.y,4.0) == 0.0) { detail *= 0.78; }
        if (pixel.x < 2.0 || pixel.x >= 14.0 || pixel.y < 1.0 || pixel.y >= 15.0 || pixel.y == 11.0) { detail *= 0.36; }
        if (pattern == 6 && pixel.x >= 7.0 && pixel.x <= 8.0 && pixel.y >= 8.0 && pixel.y <= 12.0) { detail = 1.7; }
    }
    float sharpness = 1.0 - smoothstep(0.04,0.14,max(length(dFdx(UV)),length(dFdy(UV))));
    ALBEDO = COLOR.rgb * mix(1.0,detail,sharpness);
    ROUGHNESS = 0.92;
}
