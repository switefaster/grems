struct Param {
    offset: vec3<u32>,
    psi_constant: f32, // b
    alpha_factor: f32,
}

var<push_constant> c_param: Param;

@group(0)
@binding(0)
var psi_x_y: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(1)
var psi_x_z: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(2)
var psi_y_x: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(3)
var psi_y_z: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(4)
var psi_z_x: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(5)
var psi_z_y: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(6)
var field_x: texture_storage_3d<r32float, read>;

@group(0)
@binding(7)
var field_y: texture_storage_3d<r32float, read>;

@group(0)
@binding(8)
var field_z: texture_storage_3d<r32float, read>;

@group(0)
@binding(9)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn update_electric_psi(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let local_h = vec3<f32>(textureLoad(field_x, field_texel).x, textureLoad(field_y, field_texel).x, textureLoad(field_z, field_texel).x);
    let x_actual_texel = vec3<i32>(field_texel.x - 1, field_texel.y, field_texel.z);
    let h_shift_x_y = textureLoad(field_y, x_actual_texel).x;
    let h_shift_x_z = textureLoad(field_z, x_actual_texel).x;
    let y_actual_texel = vec3<i32>(field_texel.x, field_texel.y - 1, field_texel.z);
    let h_shift_y_x = textureLoad(field_x, y_actual_texel).x;
    let h_shift_y_z = textureLoad(field_z, y_actual_texel).x;
    let z_actual_texel = vec3<i32>(field_texel.x, field_texel.y, field_texel.z - 1);
    let h_shift_z_x = textureLoad(field_x, z_actual_texel).x;
    let h_shift_z_y = textureLoad(field_y, z_actual_texel).x;
    let constant = textureLoad(constants_map, field_texel).xy;
    let c = (c_param.psi_constant - 1.0) * c_param.alpha_factor;
    let new_psi_x_y = textureLoad(psi_x_y, pml_texel).x * c_param.psi_constant + (local_h.z - h_shift_y_z) * constant.x * c;
    let new_psi_x_z = textureLoad(psi_x_z, pml_texel).x * c_param.psi_constant + (local_h.y - h_shift_z_y) * constant.x * c;
    let new_psi_y_x = textureLoad(psi_y_x, pml_texel).x * c_param.psi_constant + (local_h.z - h_shift_x_z) * constant.x * c;
    let new_psi_y_z = textureLoad(psi_y_z, pml_texel).x * c_param.psi_constant + (local_h.x - h_shift_z_x) * constant.x * c;
    let new_psi_z_x = textureLoad(psi_z_x, pml_texel).x * c_param.psi_constant + (local_h.y - h_shift_x_y) * constant.x * c;
    let new_psi_z_y = textureLoad(psi_z_y, pml_texel).x * c_param.psi_constant + (local_h.x - h_shift_y_x) * constant.x * c;
    textureStore(psi_x_y, pml_texel, vec4<f32>(new_psi_x_y, 0.0, 0.0, 1.0));
    textureStore(psi_x_z, pml_texel, vec4<f32>(new_psi_x_z, 0.0, 0.0, 1.0));
    textureStore(psi_y_x, pml_texel, vec4<f32>(new_psi_y_x, 0.0, 0.0, 1.0));
    textureStore(psi_y_z, pml_texel, vec4<f32>(new_psi_y_z, 0.0, 0.0, 1.0));
    textureStore(psi_z_x, pml_texel, vec4<f32>(new_psi_z_x, 0.0, 0.0, 1.0));
    textureStore(psi_z_y, pml_texel, vec4<f32>(new_psi_z_y, 0.0, 0.0, 1.0));
}

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_psi(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let pml_texel = vec3<i32>(global_invocation_id);
    let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
    let local_e = vec3<f32>(textureLoad(field_x, field_texel).x, textureLoad(field_y, field_texel).x, textureLoad(field_z, field_texel).x);
    let x_actual_texel = vec3<i32>(field_texel.x + 1, field_texel.y, field_texel.z);
    let e_shift_x_y = textureLoad(field_y, x_actual_texel).x;
    let e_shift_x_z = textureLoad(field_z, x_actual_texel).x;
    let y_actual_texel = vec3<i32>(field_texel.x, field_texel.y + 1, field_texel.z);
    let e_shift_y_x = textureLoad(field_x, y_actual_texel).x;
    let e_shift_y_z = textureLoad(field_z, y_actual_texel).x;
    let z_actual_texel = vec3<i32>(field_texel.x, field_texel.y, field_texel.z + 1);
    let e_shift_z_x = textureLoad(field_x, z_actual_texel).x;
    let e_shift_z_y = textureLoad(field_y, z_actual_texel).x;
    let constant = textureLoad(constants_map, field_texel).xy;
    let c = (c_param.psi_constant - 1.0) * c_param.alpha_factor;
    let new_psi_x_y = textureLoad(psi_x_y, pml_texel).x * c_param.psi_constant - (local_e.z - e_shift_y_z) * constant.x * c;
    let new_psi_x_z = textureLoad(psi_x_z, pml_texel).x * c_param.psi_constant - (local_e.y - e_shift_z_y) * constant.x * c;
    let new_psi_y_x = textureLoad(psi_y_x, pml_texel).x * c_param.psi_constant - (local_e.z - e_shift_x_z) * constant.x * c;
    let new_psi_y_z = textureLoad(psi_y_z, pml_texel).x * c_param.psi_constant - (local_e.x - e_shift_z_x) * constant.x * c;
    let new_psi_z_x = textureLoad(psi_z_x, pml_texel).x * c_param.psi_constant - (local_e.y - e_shift_x_y) * constant.x * c;
    let new_psi_z_y = textureLoad(psi_z_y, pml_texel).x * c_param.psi_constant - (local_e.x - e_shift_y_x) * constant.x * c;
    textureStore(psi_x_y, pml_texel, vec4<f32>(new_psi_x_y, 0.0, 0.0, 1.0));
    textureStore(psi_x_z, pml_texel, vec4<f32>(new_psi_x_z, 0.0, 0.0, 1.0));
    textureStore(psi_y_x, pml_texel, vec4<f32>(new_psi_y_x, 0.0, 0.0, 1.0));
    textureStore(psi_y_z, pml_texel, vec4<f32>(new_psi_y_z, 0.0, 0.0, 1.0));
    textureStore(psi_z_x, pml_texel, vec4<f32>(new_psi_z_x, 0.0, 0.0, 1.0));
    textureStore(psi_z_y, pml_texel, vec4<f32>(new_psi_z_y, 0.0, 0.0, 1.0));
}
