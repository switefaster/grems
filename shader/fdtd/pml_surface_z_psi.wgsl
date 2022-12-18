struct Param {
    total_dimension: vec3<u32>, // total field dimension
    dimension: vec3<u32>, // PML region dimension
    offset: vec3<u32>,
    psi_constant: f32 // b
}

var<push_constant> c_param: Param;

@group(0)
@binding(0)
var psi_x_z: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(1)
var psi_y_z: texture_storage_3d<r32float, read_write>;

@group(0)
@binding(2)
var field_x: texture_storage_3d<r32float, read>;

@group(0)
@binding(3)
var field_y: texture_storage_3d<r32float, read>;

@group(0)
@binding(4)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn update_electric_psi(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z {
        let pml_texel = vec3<i32>(global_invocation_id);
        let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
        let local_h = vec3<f32>(textureLoad(field_x, field_texel).x, textureLoad(field_y, field_texel).x, 0.0);
        var h_shift_z_x = 0.0;
        var h_shift_z_y = 0.0;
        if field_texel.z > 0 {
            let actual_texel = vec3<i32>(field_texel.x, field_texel.y, field_texel.z - 1);
            h_shift_z_x = textureLoad(field_x, actual_texel).x;
            h_shift_z_y = textureLoad(field_y, actual_texel).x;
        }
        let constant = textureLoad(constants_map, field_texel).xy;
        let c = c_param.psi_constant - 1.0;
        let new_psi_x_z = textureLoad(psi_x_z, pml_texel).x * c_param.psi_constant + (local_h.y - h_shift_z_y) * constant.x * c;
        let new_psi_y_z = textureLoad(psi_y_z, pml_texel).x * c_param.psi_constant + (local_h.x - h_shift_z_x) * constant.x * c;
        textureStore(psi_x_z, pml_texel, vec4<f32>(new_psi_x_z, 0.0, 0.0, 1.0));
        textureStore(psi_y_z, pml_texel, vec4<f32>(new_psi_y_z, 0.0, 0.0, 1.0));
    }
}

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_psi(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z {
        let pml_texel = vec3<i32>(global_invocation_id);
        let field_texel = vec3<i32>(global_invocation_id + c_param.offset);
        let local_e = vec3<f32>(textureLoad(field_x, field_texel).x, textureLoad(field_y, field_texel).x, 0.0);
        var e_shift_z_x = 0.0;
        var e_shift_z_y = 0.0;
        if field_texel.z < i32(c_param.total_dimension.z) - 1 {
            let actual_texel = vec3<i32>(field_texel.x, field_texel.y, field_texel.z + 1);
            e_shift_z_x = textureLoad(field_x, actual_texel).x;
            e_shift_z_y = textureLoad(field_y, actual_texel).x;
        }
        let constant = textureLoad(constants_map, field_texel).xy;
        let c = c_param.psi_constant - 1.0;
        let new_psi_x_z = textureLoad(psi_x_z, pml_texel).x * c_param.psi_constant - (local_e.y - e_shift_z_y) * constant.x * c;
        let new_psi_y_z = textureLoad(psi_y_z, pml_texel).x * c_param.psi_constant - (local_e.x - e_shift_z_x) * constant.x * c;
        textureStore(psi_x_z, pml_texel, vec4<f32>(new_psi_x_z, 0.0, 0.0, 1.0));
        textureStore(psi_y_z, pml_texel, vec4<f32>(new_psi_y_z, 0.0, 0.0, 1.0));
    }
}