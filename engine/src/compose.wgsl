struct Camera { eye:vec4f, right:vec4f, up:vec4f, forward:vec4f };
struct Params { camera:Camera, previous:Camera, viewport:vec4f, old_viewport:vec4f, size:vec4u, scene:vec4u };
struct History { position:vec4f, normal:vec4f, indirect:vec4f, direct:vec4f };
@group(0) @binding(0) var<uniform> params:Params;
@group(0) @binding(1) var positions:texture_2d<f32>;
@group(0) @binding(2) var normals:texture_2d<f32>;
@group(0) @binding(3) var albedo:texture_2d<f32>;
@group(0) @binding(4) var<storage,read> history:array<History>;
// Mode and output transfer are independent from the scene's temporal state.
struct Display { mode:u32, srgb:u32, padding:vec2u };
@group(0) @binding(5) var<uniform> display:Display;
@vertex fn vs_fullscreen(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {
    let p=vec2f(f32((i<<1u)&2u),f32(i&2u));
    return vec4f(p*2.0-1.0,0,1);
}
fn tone_map(x:vec3f)->vec3f { return x/(vec3f(1)+x); }
fn encode_srgb(x:vec3f)->vec3f {
    return select(1.055*pow(x,vec3f(1.0/2.4))-0.055,x*12.92,x<=vec3f(0.0031308));
}
@fragment fn fs_compose(@builtin(position) pixel:vec4f)->@location(0) vec4f {
    let p=textureLoad(positions,vec2i(pixel.xy),0);
    var color=vec3f(0.55,0.68,0.72);
    if p.w>0.0 {
        let n=textureLoad(normals,vec2i(pixel.xy),0).xyz;
        let base=textureLoad(albedo,vec2i(pixel.xy),0).xyz;
        let stride=params.scene.x;
        let half_size=(params.size.xy+vec2u(stride-1u))/stride;
        let center=vec2i(pixel.xy/f32(stride));
        var indirect=vec3f(0); var direct=vec3f(0); var weight=0.0;
        // Joint bilateral reconstruction: never mix opposite normals or wall planes.
        for(var y=-2;y<=2;y++) { for(var x=-2;x<=2;x++) {
            let q=center+vec2i(x,y);
            if any(q<vec2i(0)) || any(q>=vec2i(half_size)) { continue; }
            let sample=history[u32(q.x)+u32(q.y)*half_size.x];
            if sample.position.w==0.0 || dot(n,sample.normal.xyz)<0.98 { continue; }
            let delta=sample.position.xyz-p.xyz;
            if abs(dot(delta,n))>0.025 { continue; }
            let radius=0.08+0.012*distance(p.xyz,params.camera.eye.xyz);
            let w=exp(-dot(delta,delta)/(radius*radius))*exp(-f32(x*x+y*y)*0.25);
            indirect+=sample.indirect.xyz*w; direct+=sample.direct.xyz*w; weight+=w;
        }}
        if weight>0.00001 { indirect/=weight; direct/=weight; }
        var light=indirect+direct;
        if display.mode==1u { light=direct; }
        if display.mode==2u { light=indirect; }
        color=tone_map(base*light);
        // Fog remains a presentation effect, outside all light-transport feedback.
        let fog=smoothstep(28.0,66.0,distance(p.xyz,params.camera.eye.xyz));
        color=mix(color,vec3f(0.263,0.420,0.477),fog);
        color=encode_srgb(color);
    }
    // The desktop often uses a non-sRGB surface; avoid a second transfer on sRGB targets.
    if display.srgb!=0u { color=select(pow((color+0.055)/1.055,vec3f(2.4)),color/12.92,color<=vec3f(0.04045)); }
    return vec4f(color,1);
}
