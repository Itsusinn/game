shader_type canvas_item;
render_mode unshaded;
uniform sampler2D screen_texture : hint_screen_texture, repeat_disable, filter_linear_mipmap;
uniform vec2 panel_size = vec2(500.0,180.0);
uniform vec2 pointer = vec2(200.0,50.0);
uniform float elevation = 0.0;
uniform vec4 tint : source_color = vec4(0.933,0.941,0.929,1.0);
float rounded(vec2 p, vec2 half_size, float r) {
	vec2 q = abs(p)-half_size+vec2(r);
	return min(max(q.x,q.y),0.0)+length(max(q,0.0))-r;
}
void fragment() {
	vec2 p = UV*(panel_size+vec2(64.0))-vec2(32.0);
	p.y += elevation*5.0;
	vec2 center = panel_size*0.5;
	float d = rounded(p-center,center,20.0);
	float shadow = exp(-max(rounded(p-center-vec2(0.0,9.0+elevation*5.0),center,20.0),0.0)/9.0)*0.20;
	float face = 1.0-smoothstep(-0.6,0.8,d);
	float edge = (1.0-smoothstep(0.0,1.8,abs(d+1.0)))*0.62;
	float reveal = exp(-length((p-pointer)/170.0))*elevation;
	vec3 backdrop = textureLod(screen_texture,SCREEN_UV,3.5).rgb;

	vec3 surface = mix(backdrop,tint.rgb,0.92) + (1.0-p.y/panel_size.y)*0.045 + reveal*0.10;
	// A lit upper rim and shaded lower bevel give the glass visible thickness.
	surface += edge*mix(-0.12,0.16,1.0-clamp(p.y/panel_size.y,0.0,1.0));
	COLOR = vec4(mix(vec3(0.04,0.05,0.07),surface,face),max(face,shadow));
}
