struct Param {
    dimension: vec3<u32>, // non-PML region dimension
    offset: vec3<u32>,
    use_pml: i32,
    position: vec3<u32>,
    size: vec3<u32>,
    strength: vec3<f32>,
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
var conjugative_field_x: texture_storage_3d<r32float, read>;

@group(0)
@binding(4)
var conjugative_field_y: texture_storage_3d<r32float, read>;

@group(0)
@binding(5)
var conjugative_field_z: texture_storage_3d<r32float, read>;

@group(0)
@binding(6)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn excite_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if global_invocation_id.x < c_param.size.x && global_invocation_id.y < c_param.size.y && global_invocation_id.z < c_param.size.z {
        let actual_texel = vec3<i32>(c_param.position + global_invocation_id);
        let prev_field = vec3<f32>(textureLoad(update_field_x, actual_texel).x, textureLoad(update_field_y, actual_texel).x, textureLoad(update_field_z, actual_texel).x);
        let new_field = prev_field + textureLoad(constants_map, actual_texel).y * c_param.strength;
        textureStore(update_field_x, actual_texel, vec4<f32>(new_field.x, 0.0, 0.0, 1.0));
        textureStore(update_field_y, actual_texel, vec4<f32>(new_field.y, 0.0, 0.0, 1.0));
        textureStore(update_field_z, actual_texel, vec4<f32>(new_field.z, 0.0, 0.0, 1.0));
    }
}

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z {
        let texel = vec3<i32>(global_invocation_id + c_param.offset);
        let constant = textureLoad(constants_map, texel).x;
        let prev_h = vec3<f32>(textureLoad(update_field_x, texel).x, textureLoad(update_field_y, texel).x, textureLoad(update_field_z, texel).x);
        let local_e = vec3<f32>(textureLoad(conjugative_field_x, texel).x, textureLoad(conjugative_field_y, texel).x, textureLoad(conjugative_field_z, texel).x);
        var e_shift_x_y = 0.0;
        var e_shift_x_z = 0.0;
        if (texel.x < i32(c_param.dimension.x) - 1) || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x + 1, texel.y, texel.z);
            e_shift_x_y = textureLoad(conjugative_field_y, actual_texel).x;
            e_shift_x_z = textureLoad(conjugative_field_z, actual_texel).x;
        }
        var e_shift_y_x = 0.0;
        var e_shift_y_z = 0.0;
        if (texel.y < i32(c_param.dimension.y) - 1) || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x, texel.y + 1, texel.z);
            e_shift_y_x = textureLoad(conjugative_field_x, actual_texel).x;
            e_shift_y_z = textureLoad(conjugative_field_z, actual_texel).x;
        }
        var e_shift_z_x = 0.0;
        var e_shift_z_y = 0.0;
        if (texel.z < i32(c_param.dimension.z) - 1) || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x, texel.y, texel.z + 1);
            e_shift_z_x = textureLoad(conjugative_field_x, actual_texel).x;
            e_shift_z_y = textureLoad(conjugative_field_y, actual_texel).x;
        }
        let diff_hx = (e_shift_z_y - local_e.y) - (e_shift_y_z - local_e.z);
        let diff_hy = (e_shift_x_z - local_e.z) - (e_shift_z_x - local_e.x);
        let diff_hz = (e_shift_y_x - local_e.x) - (e_shift_x_y - local_e.y);
        var store_value = prev_h + constant * vec3<f32>(diff_hx, diff_hy, diff_hz);
        if c_param.use_pml == 0 {
            if texel.x == i32(c_param.dimension.x) - 1 {
                store_value.y = 0.0;
                store_value.z = 0.0;
            }
            if texel.y == i32(c_param.dimension.y) - 1 {
                store_value.x = 0.0;
                store_value.z = 0.0;
            }
            if texel.z == i32(c_param.dimension.z) - 1 {
                store_value.x = 0.0;
                store_value.y = 0.0;
            }
        }
        textureStore(update_field_x, texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
        textureStore(update_field_y, texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
        textureStore(update_field_z, texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
    }
}

@compute
@workgroup_size(8, 8, 8)
fn update_electric_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z {
        let texel = vec3<i32>(global_invocation_id + c_param.offset);
        let constant = textureLoad(constants_map, texel).x;
        let prev_e = vec3<f32>(textureLoad(update_field_x, texel).x, textureLoad(update_field_y, texel).x, textureLoad(update_field_z, texel).x);
        let local_h = vec3<f32>(textureLoad(conjugative_field_x, texel).x, textureLoad(conjugative_field_y, texel).x, textureLoad(conjugative_field_z, texel).x);
        var h_shift_x_y = 0.0;
        var h_shift_x_z = 0.0;
        if texel.x > 0 || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x - 1, texel.y, texel.z);
            h_shift_x_y = textureLoad(conjugative_field_y, actual_texel).x;
            h_shift_x_z = textureLoad(conjugative_field_z, actual_texel).x;
        }
        var h_shift_y_x = 0.0;
        var h_shift_y_z = 0.0;
        if texel.y > 0 || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x, texel.y - 1, texel.z);
            h_shift_y_x = textureLoad(conjugative_field_x, actual_texel).x;
            h_shift_y_z = textureLoad(conjugative_field_z, actual_texel).x;
        }
        var h_shift_z_x = 0.0;
        var h_shift_z_y = 0.0;
        if texel.z > 0 || c_param.use_pml == 1 {
            let actual_texel = vec3<i32>(texel.x, texel.y, texel.z - 1);
            h_shift_z_x = textureLoad(conjugative_field_x, actual_texel).x;
            h_shift_z_y = textureLoad(conjugative_field_y, actual_texel).x;
        }
        let diff_ex = (local_h.z - h_shift_y_z) - (local_h.y - h_shift_z_y);
        let diff_ey = (local_h.x - h_shift_z_x) - (local_h.z - h_shift_x_z);
        let diff_ez = (local_h.y - h_shift_x_y) - (local_h.x - h_shift_y_x);
        var store_value = prev_e + constant * vec3<f32>(diff_ex, diff_ey, diff_ez);
        if c_param.use_pml == 0 {
            if texel.x == i32(c_param.dimension.x) - 1 {
                store_value.x = 0.0;
            }
            if texel.y == i32(c_param.dimension.y) - 1 {
                store_value.y = 0.0;
            }
            if texel.z == i32(c_param.dimension.z) - 1 {
                store_value.z = 0.0;
            }
        }
        textureStore(update_field_x, texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
        textureStore(update_field_y, texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
        textureStore(update_field_z, texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
    }
}
