// Original WGSL adaptation of kajiya's final-gather + recurrent irradiance-cache
// architecture. See engine/LIGHTING.md for deliberate differences and references.
struct Camera { eye:vec4f, right:vec4f, up:vec4f, forward:vec4f };
struct Params {
    camera:Camera, previous:Camera,
    viewport:vec4f, old_viewport:vec4f,
    size:vec4u, // Full width, height, stochastic frame, valid history.
    scene:vec4u, // Trace pixel stride, total nodes, static triangles, changed actors.
};
struct Node { low:vec4f, high:vec4f, info:vec4u };
struct Triangle { a:vec4f, b:vec4f, c:vec4f, normal:vec4f, color:vec4f };
struct History { position:vec4f, normal:vec4f, indirect:vec4f, direct:vec4f };
@group(0) @binding(0) var<uniform> params:Params;
@group(0) @binding(1) var<storage,read> nodes:array<Node>;
@group(0) @binding(2) var<storage,read> triangles:array<Triangle>;
@group(0) @binding(3) var<storage,read> cache_read:array<vec4f>;
@group(0) @binding(4) var<storage,read_write> cache_write:array<vec4f>;
@group(0) @binding(5) var positions:texture_2d<f32>;
@group(0) @binding(6) var normals:texture_2d<f32>;
@group(0) @binding(7) var<storage,read> old_history:array<History>;
@group(0) @binding(8) var<storage,read_write> new_history:array<History>;

const PI:f32 = 3.14159265359;
const MISS:u32 = 0xffffffffu;
const SUN:vec3f = vec3f(-0.4466625, 0.8436958, 0.297775);
const SUN_LIGHT:vec3f = vec3f(2.6, 2.35, 1.95);
const BIAS:f32 = 0.003;
fn random(state:ptr<function,u32>)->f32 {
    *state = *state * 747796405u + 2891336453u;
    let word = ((*state >> ((*state >> 28u) + 4u)) ^ *state) * 277803737u;
    return f32((word >> 22u) ^ word) / 4294967296.0;
}
fn hemisphere(n:vec3f, state:ptr<function,u32>)->vec3f {
    let r = sqrt(random(state));
    let phi = 2.0 * PI * random(state);
    let tangent = normalize(cross(select(vec3f(0,1,0), vec3f(1,0,0), abs(n.y)>0.9), n));
    return tangent*(r*cos(phi)) + cross(n,tangent)*(r*sin(phi)) + n*sqrt(max(0.0,1.0-r*r));
}
fn sky(direction:vec3f)->vec3f {
    return mix(vec3f(0.35,0.42,0.52), vec3f(0.55,0.75,1.05), clamp(direction.y,0.0,1.0));
}
fn box_hit(origin:vec3f, direction:vec3f, node:Node, distance:f32)->bool {
    // Explicit parallel-axis handling avoids 0*infinity NaNs on block faces.
    var near=0.0;
    var far=distance;
    for (var axis=0u; axis<3u; axis++) {
        if abs(direction[axis]) < 0.0000001 {
            if origin[axis]<node.low[axis] || origin[axis]>node.high[axis] { return false; }
        } else {
            let a=(node.low[axis]-origin[axis])/direction[axis];
            let b=(node.high[axis]-origin[axis])/direction[axis];
            near=max(near,min(a,b)); far=min(far,max(a,b));
        }
    }
    return near<=far;
}
struct Hit { distance:f32, triangle:u32 };
fn trace(origin:vec3f, direction:vec3f, any_hit:bool)->Hit {
    var hit=Hit(10000.0,MISS);
    // Static and dynamic trees are concatenated; their escape links are absolute.
    var index=0u;
    loop {
        if index>=params.scene.y { break; }
        let node=nodes[index];
        if !box_hit(origin,direction,node,hit.distance) { index=node.info.z; continue; }
        if node.info.y==0u { index++; continue; }
        for(var j=0u; j<node.info.y; j++) {
            let id=node.info.x+j;
            let t=triangles[id];
            let e1=t.b.xyz-t.a.xyz; let e2=t.c.xyz-t.a.xyz;
            let p=cross(direction,e2); let det=dot(e1,p);
            if abs(det)<0.0000001 { continue; }
            let inv=1.0/det; let s=origin-t.a.xyz;
            let u=dot(s,p)*inv;
            let q=cross(s,e1); let v=dot(direction,q)*inv;
            let d=dot(e2,q)*inv;
            if u>=0.0 && v>=0.0 && u+v<=1.0 && d>0.0001 && d<hit.distance {
                hit=Hit(d,id);
                if any_hit { return hit; }
            }
        }
        index=node.info.z;
    }
    return hit;
}
fn direct_light(p:vec3f,n:vec3f,state:ptr<function,u32>)->vec3f {
    // Finite sun disk. Temporal and bilateral filters remove stochastic edges.
    let r=sqrt(random(state))*0.009;
    let phi=random(state)*2.0*PI;
    let t=normalize(cross(SUN,vec3f(0,1,0)));
    let direction=normalize(SUN+t*r*cos(phi)+cross(SUN,t)*r*sin(phi));
    let cosine=max(0.0,dot(n,direction));
    if cosine==0.0 { return vec3f(0); }
    if trace(p+n*BIAS,direction,true).triangle!=MISS { return vec3f(0); }
    return SUN_LIGHT*cosine;
}
fn incoming(p:vec3f,direction:vec3f,state:ptr<function,u32>)->vec3f {
    let hit=trace(p,direction,false);
    if hit.triangle==MISS { return sky(direction); }
    let triangle=triangles[hit.triangle];
    let n=triangle.normal.xyz;
    // Back faces block rays, but do not emit light into closed geometry.
    if dot(n,direction)>=0.0 { return vec3f(0); }
    let point=p+direction*hit.distance;
    var tail=vec3f(0);
    if hit.triangle<params.scene.z { tail=cache_read[hit.triangle].xyz; }
    // Cache stores irradiance/pi. A cosine-weighted ray estimates this directly.
    return clamp(triangle.color.xyz,vec3f(0),vec3f(0.9))*(direct_light(point,n,state)+tail);
}

@compute @workgroup_size(64)
fn update_cache(@builtin(global_invocation_id) id:vec3u) {
    if id.x>=params.scene.z { return; }
    let t=triangles[id.x];
    var rng=id.x*9781u+params.size.z*6271u+0x9e3779b9u;
    // Sample a new location on each surface every frame; no cross-wall interpolation.
    let r=sqrt(random(&rng)); let s=random(&rng);
    let p=t.a.xyz*(1.0-r)+t.b.xyz*(r*(1.0-s))+t.c.xyz*(r*s)+t.normal.xyz*BIAS;
    var sum=vec3f(0);
    for(var i=0u;i<2u;i++) { sum+=incoming(p,hemisphere(t.normal.xyz,&rng),&rng); }
    let old=cache_read[id.x];
    let age=min(old.w,31.0);
    let value=mix(old.xyz,sum*0.5,1.0/(age+1.0));
    // Ping-pong avoids undefined cross-invocation feedback and permits GPU validation.
    cache_write[id.x]=vec4f(value,age+1.0);
}

fn previous_pixel(p:vec3f)->vec2i {
    let d=p-params.previous.eye.xyz;
    let z=dot(d,params.previous.forward.xyz);
    if z<=0.05 { return vec2i(-1); }
    let ndc=vec2f(dot(d,params.previous.right.xyz)/params.previous.eye.w,
                  -dot(d,params.previous.up.xyz))/(z*params.previous.right.w);
    let pixel=params.old_viewport.xy+(ndc*0.5+0.5)*params.old_viewport.zw;
    return vec2i(floor(pixel/f32(params.scene.x)));
}

@compute @workgroup_size(8,8)
fn gather(@builtin(global_invocation_id) id:vec3u) {
    let stride=params.scene.x;
    let half_size=(params.size.xy+vec2u(stride-1u))/stride;
    if any(id.xy>=half_size) { return; }
    let index=id.x+id.y*half_size.x;
    // Normally 2x2; larger tiles keep very high-DPI history within device limits.
    let offset=vec2u(params.size.z%stride,(params.size.z/stride)%stride);
    let pixel=min(id.xy*stride+offset,params.size.xy-vec2u(1));
    let p=textureLoad(positions,vec2i(pixel),0);
    let n=textureLoad(normals,vec2i(pixel),0);
    if p.w==0.0 { new_history[index]=History(vec4f(0),vec4f(0),vec4f(0),vec4f(0)); return; }
    var rng=index*1973u+params.size.z*9277u+89173u;
    let indirect=incoming(p.xyz+n.xyz*BIAS,hemisphere(n.xyz,&rng),&rng);
    let direct=direct_light(p.xyz,n.xyz,&rng);
    var result=History(p,n,vec4f(indirect,1),vec4f(direct,1));
    if params.size.w!=0u {
        let old_pixel=previous_pixel(p.xyz);
        var best=0.25;
        var found=false;
        var previous:History;
        for(var y=-1;y<=1;y++) { for(var x=-1;x<=1;x++) {
            let q=old_pixel+vec2i(x,y);
            if any(q<vec2i(0)) || any(q>=vec2i(half_size)) { continue; }
            let old=old_history[u32(q.x)+u32(q.y)*half_size.x];
            let delta=distance(p.xyz,old.position.xyz);
            // Surface distance + normal rejection avoids dragging light across walls.
            let threshold=min(0.25,0.035+0.002*distance(p.xyz,params.camera.eye.xyz));
            if old.position.w>0.0 && dot(old.normal.xyz,n.xyz)>0.98 && delta<threshold && delta<best {
                best=delta; previous=old; found=true;
            }
        }}
        if found {
            let age=min(previous.indirect.w,select(31.0,3.0,params.scene.w!=0u));
            result.indirect=vec4f(mix(previous.indirect.xyz,indirect,1.0/(age+1.0)),age+1.0);
            // Shadows react faster than diffuse history when an actor moves.
            let direct_age=min(previous.direct.w,3.0);
            result.direct=vec4f(mix(previous.direct.xyz,direct,1.0/(direct_age+1.0)),direct_age+1.0);
        }
    }
    new_history[index]=result;
}
