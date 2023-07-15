struct Param {
    offset: vec3<u32>,
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

@group(1)
@binding(0)
var psi_y_x: texture_storage_3d<r32float, read>;

@group(1)
@binding(1)
var psi_z_x: texture_storage_3d<r32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let prev_h = vec3<f32>(0.0, textureLoad(update_field_y, field_texel).x, textureLoad(update_field_z, field_texel).x);
    let p_y_x = textureLoad(psi_y_x, pml_texel).x;
    let p_z_x = textureLoad(psi_z_x, pml_texel).x;
    let store_value = prev_h + vec3<f32>(0.0, p_y_x, -p_z_x);

    textureStore(update_field_y, field_texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, field_texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}

@compute
@workgroup_size(8, 8, 8)
fn update_electric_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let prev_e = vec3<f32>(0.0, textureLoad(update_field_y, field_texel).x, textureLoad(update_field_z, field_texel).x);
    let p_y_x = textureLoad(psi_y_x, pml_texel).x;
    let p_z_x = textureLoad(psi_z_x, pml_texel).x;
    let store_value = prev_e + vec3<f32>(0.0, -p_y_x, p_z_x);

    textureStore(update_field_y, field_texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, field_texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}
