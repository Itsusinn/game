shader_type canvas_item;
render_mode unshaded;
uniform sampler2D screen_texture : hint_screen_texture, repeat_disable, filter_linear_mipmap;

void fragment() {
	vec3 scene = textureLod(screen_texture, SCREEN_UV, 2.2).rgb;
	float grey = dot(scene, vec3(0.2126, 0.7152, 0.0722));
	vec3 muted = mix(scene, vec3(grey), 0.25);
	COLOR = vec4(mix(muted, vec3(0.035, 0.045, 0.035), 0.55), 1.0);
}
