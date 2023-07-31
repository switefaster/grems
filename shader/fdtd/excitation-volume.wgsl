struct Param {
    size: vec3<u32>,
    strength: vec3<f32>,
    position: vec3<u32>,
}

var<push_constant> c_param: Param;

@group(0)
@binding(0)
var update_field_x: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(1)
var update_field_y: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(2)
var update_field_z: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(3)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(WORKGROUP_X, WORKGROUP_Y, WORKGROUP_Z)
fn excite_field_volume(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let actual_texel = vec3<i32>(c_param.position + global_invocation_id);
    let prev_field = vec3<f32>(textureLoad(update_field_x, actual_texel).x, textureLoad(update_field_y, actual_texel).x, textureLoad(update_field_z, actual_texel).x);
    let new_field = prev_field + textureLoad(constants_map, actual_texel).y * c_param.strength * f32(global_invocation_id.x < c_param.size.x && global_invocation_id.y < c_param.size.y && global_invocation_id.z < c_param.size.z);
    textureStore(update_field_x, actual_texel, vec4<f32>(new_field.x, 0.0, 0.0, 1.0));
    textureStore(update_field_y, actual_texel, vec4<f32>(new_field.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, actual_texel, vec4<f32>(new_field.z, 0.0, 0.0, 1.0));
}
