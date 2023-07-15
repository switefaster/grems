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
var psi_x_y: texture_storage_3d<r32float, read>;

@group(1)
@binding(1)
var psi_x_z: texture_storage_3d<r32float, read>;

@group(1)
@binding(2)
var psi_y_x: texture_storage_3d<r32float, read>;

@group(1)
@binding(3)
var psi_y_z: texture_storage_3d<r32float, read>;

@group(1)
@binding(4)
var psi_z_x: texture_storage_3d<r32float, read>;

@group(1)
@binding(5)
var psi_z_y: texture_storage_3d<r32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let prev_h = vec3<f32>(textureLoad(update_field_x, field_texel).x, textureLoad(update_field_y, field_texel).x, textureLoad(update_field_z, field_texel).x);
    let p_x_y = textureLoad(psi_x_y, pml_texel).x;
    let p_x_z = textureLoad(psi_x_z, pml_texel).x;
    let p_y_x = textureLoad(psi_y_x, pml_texel).x;
    let p_y_z = textureLoad(psi_y_z, pml_texel).x;
    let p_z_x = textureLoad(psi_z_x, pml_texel).x;
    let p_z_y = textureLoad(psi_z_y, pml_texel).x;
    let diff_psi_hx = p_x_z - p_x_y;
    let diff_psi_hy = p_y_x - p_y_z;
    let diff_psi_hz = p_z_y - p_z_x;
    let store_value = prev_h + vec3<f32>(diff_psi_hx, diff_psi_hy, diff_psi_hz);

    textureStore(update_field_x, field_texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
    textureStore(update_field_y, field_texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, field_texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}

@compute
@workgroup_size(8, 8, 8)
fn update_electric_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let prev_e = vec3<f32>(textureLoad(update_field_x, field_texel).x, textureLoad(update_field_y, field_texel).x, textureLoad(update_field_z, field_texel).x);
    let p_x_y = textureLoad(psi_x_y, pml_texel).x;
    let p_x_z = textureLoad(psi_x_z, pml_texel).x;
    let p_y_x = textureLoad(psi_y_x, pml_texel).x;
    let p_y_z = textureLoad(psi_y_z, pml_texel).x;
    let p_z_x = textureLoad(psi_z_x, pml_texel).x;
    let p_z_y = textureLoad(psi_z_y, pml_texel).x;
    let diff_psi_ex = p_x_y - p_x_z;
    let diff_psi_ey = p_y_z - p_y_x;
    let diff_psi_ez = p_z_x - p_z_y;
    let store_value = prev_e + vec3<f32>(diff_psi_ex, diff_psi_ey, diff_psi_ez);

    textureStore(update_field_x, field_texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
    textureStore(update_field_y, field_texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, field_texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}
