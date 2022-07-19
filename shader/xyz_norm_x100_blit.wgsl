struct SliceParam {
    slice_position: f32,
    slice_mode: u32,
};

var<push_constant> c_param: SliceParam;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@group(0)
@binding(0)
var t_texture: texture_3d<f32>;

@group(0)
@binding(1)
var s_sampler: sampler;

@fragment
fn fs_main(fin: VertexOut) -> @location(0) vec4<f32> {
    var field: vec3<f32>;
    if (c_param.slice_mode == 0u) {
        field = textureSample(t_texture, s_sampler, vec3<f32>(fin.tex_coord, c_param.slice_position)).rgb * 10000000.0;
    } else if (c_param.slice_mode == 1u) {
        field = textureSample(t_texture, s_sampler, vec3<f32>(fin.tex_coord.x, c_param.slice_position, fin.tex_coord.y)).rgb * 100000.0;
    } else if (c_param.slice_mode == 2u) {
        field = textureSample(t_texture, s_sampler, vec3<f32>(c_param.slice_position, fin.tex_coord.x, fin.tex_coord.y)).rgb * 100000.0;
    } else {
        discard;
    }
    var norm: f32 = length(field);
    return vec4<f32>(norm, norm, norm, 1.0);
}
