struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) tex_coord: vec2<f32>) -> VertexOut {
    var vout: VertexOut;
    vout.position = vec4<f32>(pos, 0.0, 1.0);
    vout.tex_coord = tex_coord;
    return vout;
}
