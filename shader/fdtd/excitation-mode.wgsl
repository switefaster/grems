struct Param {
    position: vec3<u32>,
    cos_t: f32,
    sin_t: f32,
    envelope: f32,
    dt: f32,
}

var<push_constant> c_param: Param;

@group(0)
@binding(0)
var mode_source_x: texture_storage_2d<rg32float, read>;

@group(0)
@binding(1)
var mode_source_y: texture_storage_2d<rg32float, read>;

@group(0)
@binding(2)
var mode_source_z: texture_storage_2d<rg32float, read>;

@group(1)
@binding(0)
var update_field_x: texture_storage_3d<r32float, read_write>;

@group(1)
@binding(1)
var update_field_y: texture_storage_3d<r32float, read_write>;

@group(1)
@binding(2)
var update_field_z: texture_storage_3d<r32float, read_write>;

@group(1)
@binding(3)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(WORKGROUP_X, WORKGROUP_Y, 1)
fn excite_field_mode(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let actual_texel = vec3<i32>(c_param.position + global_invocation_id);
    let source_texel = vec2<i32>(global_invocation_id.xy);
    let complex_x = textureLoad(mode_source_x, source_texel).xy;
    let complex_y = textureLoad(mode_source_y, source_texel).xy;
    let complex_z = textureLoad(mode_source_z, source_texel).xy;

    let prev_field = vec3<f32>(textureLoad(update_field_x, actual_texel).x, textureLoad(update_field_y, actual_texel).x, textureLoad(update_field_z, actual_texel).x);

    let x = complex_x.x * c_param.cos_t + complex_x.y * c_param.sin_t;
    let y = complex_y.x * c_param.cos_t + complex_y.y * c_param.sin_t;
    let z = complex_z.x * c_param.cos_t + complex_z.y * c_param.sin_t;

    textureStore(update_field_x, actual_texel, vec4<f32>(prev_field.x + x * c_param.envelope * c_param.dt, 0.0, 0.0, 1.0));
    textureStore(update_field_y, actual_texel, vec4<f32>(prev_field.y + y * c_param.envelope * c_param.dt, 0.0, 0.0, 1.0));
    textureStore(update_field_z, actual_texel, vec4<f32>(prev_field.z + z * c_param.envelope * c_param.dt, 0.0, 0.0, 1.0));
}
