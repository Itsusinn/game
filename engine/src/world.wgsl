struct Camera { eye:vec4f, right:vec4f, up:vec4f, forward:vec4f };
@group(0) @binding(0) var<uniform> camera:Camera;
struct Out {
    @builtin(position) position:vec4f,
    @location(0) world:vec3f,
    @location(1) normal:vec3f,
    @location(2) color:vec3f,
    @location(3) uv:vec2f,
    @location(4) @interpolate(flat) pattern:u32,
};
@vertex fn vs_main(@location(0) position:vec3f,@location(1) normal:vec3f,@location(2) color:vec3f,@location(3) uv:vec2f,@location(4) pattern:u32)->Out {
    let d=position-camera.eye.xyz;
    let z=dot(d,camera.forward.xyz);
    var out:Out;
    out.position=vec4f(dot(d,camera.right.xyz)/camera.right.w/camera.eye.w,dot(d,camera.up.xyz)/camera.right.w,120.0/119.95*z-6.0/119.95,z);
    out.world=position;out.normal=normal;out.color=color;out.uv=uv;out.pattern=pattern;return out;
}
fn hash(p:vec3f)->f32 { return fract(sin(dot(p,vec3f(127.1,311.7,74.7)))*43758.5453); }
@fragment fn fs_main(in:Out)->@location(0) vec4f {
    let pixel=floor(in.uv*16.0);
    let seed=floor(in.world-in.normal*0.01);
    let noise=hash(vec3f(pixel,0.0)+seed*19.0);
    var texture=0.91+noise*0.16;
    if in.pattern==0u {texture=1.0;}
    if in.pattern==1u {texture=0.88+hash(vec3f(floor(in.uv.x*8.0),floor(in.uv.y*2.0),seed.x+seed.z))*0.2;}
    if in.pattern==2u {
        let row=floor(in.uv.y*2.0);let p=fract(in.uv*vec2f(2.0,2.0)+vec2f(row*0.5,0.0));
        if p.x<0.045||p.y<0.055 {texture*=0.74;}
    }
    if in.pattern==4u {texture=0.75+noise*0.35;}
    let detail=1.0-smoothstep(0.04,0.14,max(length(dpdx(in.uv)),length(dpdy(in.uv))));
    texture=mix(1.0,texture,detail);
    let sun=normalize(vec3f(-0.45,0.85,0.3));
    let lighting=0.62+0.32*max(dot(in.normal,sun),0.0)+0.06*max(in.normal.y,0.0);
    var color=in.color*texture*lighting;
    let fog=smoothstep(28.0,66.0,length(in.world-camera.eye.xyz));
    color=mix(color,vec3f(0.55,0.68,0.72),fog);
    return vec4f(color,1.0);
}
