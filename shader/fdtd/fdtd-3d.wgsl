struct Param {
    dimension: vec3<u32>,
    position: vec3<u32>,
    size: vec3<u32>,
    strength: vec3<f32>,
}

var<push_constant> c_param: Param;

@group(0)
@binding(0)
var update_field: texture_storage_3d<rgba32float, read_write>;

@group(0)
@binding(1)
var conjugative_field: texture_storage_3d<rgba32float, read>;

@group(0)
@binding(2)
var constants_map: texture_storage_3d<rg32float, read>;

@compute
@workgroup_size(8, 8, 8)
fn excite_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if (global_invocation_id.x < c_param.size.x && global_invocation_id.y < c_param.size.y && global_invocation_id.z < c_param.size.z) {
        let actual_texel = vec3<i32>(c_param.position + global_invocation_id);
        let prev_field = textureLoad(update_field, actual_texel).xyz;
        let new_field = prev_field + textureLoad(constants_map, actual_texel).y * c_param.strength;
        textureStore(update_field, actual_texel, vec4<f32>(new_field, 1.0));
    }
}

@compute
@workgroup_size(8, 8, 8)
fn update_magnetic_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if (global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z) {
        let texel = vec3<i32>(global_invocation_id);
        let constant = textureLoad(constants_map, texel).x;
        let prev_h = textureLoad(update_field, texel).xyz;
        let local_e = textureLoad(conjugative_field, texel).xyz;
        var e_shift_x = vec3<f32>(0.0);
        if (texel.x < i32(c_param.dimension.x) - 1) {
            e_shift_x = textureLoad(conjugative_field, vec3<i32>(texel.x + 1, texel.y, texel.z)).xyz;
        }
        var e_shift_y = vec3<f32>(0.0);
        if (texel.y < i32(c_param.dimension.y) - 1) {
            e_shift_y = textureLoad(conjugative_field, vec3<i32>(texel.x, texel.y + 1, texel.z)).xyz;
        }
        var e_shift_z = vec3<f32>(0.0);
        if (texel.z < i32(c_param.dimension.z) - 1) {
            e_shift_z = textureLoad(conjugative_field, vec3<i32>(texel.x, texel.y, texel.z + 1)).xyz;
        }
        let diff_hx = (e_shift_z.y - local_e.y) - (e_shift_y.z - local_e.z);
        let diff_hy = (e_shift_x.z - local_e.z) - (e_shift_z.x - local_e.x);
        let diff_hz = (e_shift_y.x - local_e.x) - (e_shift_x.y - local_e.y);
        var store_value = prev_h + constant * vec3<f32>(diff_hx, diff_hy, diff_hz);
        if (texel.x == i32(c_param.dimension.x) - 1) {
            store_value.y = 0.0;
            store_value.z = 0.0;
        }
        if (texel.y == i32(c_param.dimension.y) - 1) {
            store_value.x = 0.0;
            store_value.z = 0.0;
        }
        if (texel.z == i32(c_param.dimension.z) - 1) {
            store_value.x = 0.0;
            store_value.y = 0.0;
        }
        textureStore(update_field, texel, vec4<f32>(store_value, 1.0));
    }
}

@compute
@workgroup_size(8, 8, 8)
fn update_electric_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    if (global_invocation_id.x < c_param.dimension.x && global_invocation_id.y < c_param.dimension.y && global_invocation_id.z < c_param.dimension.z) {
        let texel = vec3<i32>(global_invocation_id);
        let constant = textureLoad(constants_map, texel).x;
        let prev_e = textureLoad(update_field, texel).xyz;
        let local_h = textureLoad(conjugative_field, texel).xyz;
        var h_shift_x = vec3<f32>(0.0);
        if (texel.x > 0) {
            h_shift_x = textureLoad(conjugative_field, vec3<i32>(texel.x - 1, texel.y, texel.z)).xyz;
        }
        var h_shift_y = vec3<f32>(0.0);
        if (texel.y > 0) {
            h_shift_y = textureLoad(conjugative_field, vec3<i32>(texel.x, texel.y - 1, texel.z)).xyz;
        }
        var h_shift_z = vec3<f32>(0.0);
        if (texel.z > 0) {
            h_shift_z = textureLoad(conjugative_field, vec3<i32>(texel.x, texel.y, texel.z - 1)).xyz;
        }
        let diff_ex = (local_h.z - h_shift_y.z) - (local_h.y - h_shift_z.y);
        let diff_ey = (local_h.x - h_shift_z.x) - (local_h.z - h_shift_x.z);
        let diff_ez = (local_h.y - h_shift_x.y) - (local_h.x - h_shift_y.x);
        var store_value = prev_e + constant * vec3<f32>(diff_ex, diff_ey, diff_ez);
        if (texel.x == i32(c_param.dimension.x) - 1) {
            store_value.x = 0.0;
        }
        if (texel.y == i32(c_param.dimension.y) - 1) {
            store_value.y = 0.0;
        }
        if (texel.z == i32(c_param.dimension.z) - 1) {
            store_value.z = 0.0;
        }
        textureStore(update_field, texel, vec4<f32>(store_value, 1.0));
    }
}
