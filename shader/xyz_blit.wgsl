struct SliceParam {
    slice_position: f32,
    slice_mode: u32,
    scaling_factor: f32,
};

var<push_constant> c_param: SliceParam;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@group(0)
@binding(0)
var t_texture_x: texture_3d<f32>;

@group(0)
@binding(1)
var t_texture_y: texture_3d<f32>;

@group(0)
@binding(2)
var t_texture_z: texture_3d<f32>;

@group(0)
@binding(3)
var s_sampler: sampler;

@fragment
fn fs_main(fin: VertexOut) -> @location(0) vec4<f32> {
    var slice = vec3<f32>(0.0);
    if (c_param.slice_mode == 0u) {
        slice = vec3<f32>(fin.tex_coord, c_param.slice_position);
    } else if (c_param.slice_mode == 1u) {
        slice = vec3<f32>(fin.tex_coord.x, c_param.slice_position, fin.tex_coord.y);
    } else if (c_param.slice_mode == 2u) {
        slice = vec3<f32>(c_param.slice_position, fin.tex_coord.x, fin.tex_coord.y);
    } else {
        discard;
    }
    return vec4<f32>(vec3<f32>(textureSample(t_texture_x, s_sampler, slice).x, textureSample(t_texture_y, s_sampler, slice).x, textureSample(t_texture_z, s_sampler, slice).x) * c_param.scaling_factor, 1.0);
}
