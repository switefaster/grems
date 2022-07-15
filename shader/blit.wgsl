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

@group(0)
@binding(0)
var t_texture: texture_3d<f32>;

@group(0)
@binding(1)
var s_sampler: sampler;

@fragment
fn fs_main(fin: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(t_texture, s_sampler, vec3<f32>(fin.tex_coord, 0.5)).rrr * 1000.0, 1.0);
}
