struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
    @location(2) @interpolate(flat) kind: u32,
};
@vertex fn vs_main(@builtin(vertex_index) index: u32, @location(0) rect: vec4f, @location(1) color: vec4f, @location(2) kind: u32) -> VertexOut {
    let points = array<vec2f, 6>(vec2f(0,0), vec2f(1,0), vec2f(0,1), vec2f(0,1), vec2f(1,0), vec2f(1,1));
    let uv = points[index];
    var out: VertexOut;
    out.position = vec4f(rect.xy + uv * rect.zw, 0, 1);
    out.uv = uv; out.color = color; out.kind = kind;
    return out;
}
@fragment fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var color = in.color;
    let p = in.uv;
    if in.kind == 1u {
        let radius = length(p - vec2f(0.5));
        if radius > 0.5 { discard; }
        if radius > 0.39 { color = vec4f(color.rgb * 0.55, color.a); }
    }
    if in.kind == 2u && (p.x < 0.04 || p.y < 0.04) { color = vec4f(color.rgb * 0.65, color.a); }
    if in.kind == 3u && p.y > 0.2 && p.y < 0.8 && abs(p.x-0.5) < (p.y-0.2)*0.65 { color = vec4f(color.rgb*0.3, color.a); }
    if in.kind == 4u && p.y > 0.2 && p.y < 0.8 && abs(p.x-0.5) < (0.8-p.y)*0.65 { color = vec4f(color.rgb*0.3, color.a); }
    if in.kind == 5u && p.x > 0.15 && p.x < 0.85 && p.y > 0.1 && p.y < 0.9 {
        if p.x < 0.22 || p.x > 0.78 || p.y < 0.17 || p.y > 0.83 { color = vec4f(0.85,0.75,0.5,color.a); }
    }
    return color;
}
