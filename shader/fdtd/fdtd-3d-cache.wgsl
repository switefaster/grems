struct Param {
    dimension: vec3<u32>, // total dimension, only required to ensure boundary
    position: vec3<u32>,
    size: vec3<u32>,
    strength: vec3<f32>,
}

var<push_constant> c_param: Param;
var<workgroup> w_cache_x: array<f32, CACHE_VOL>;
var<workgroup> w_cache_y: array<f32, CACHE_VOL>;
var<workgroup> w_cache_z: array<f32, CACHE_VOL>;

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
@workgroup_size(WORKGROUP_X, WORKGROUP_Y, WORKGROUP_Z)
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
@workgroup_size(WORKGROUP_X, WORKGROUP_Y, WORKGROUP_Z)
fn update_magnetic_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(local_invocation_id) local_invocation_id: vec3<u32>, @builtin(local_invocation_index) local_invocation_index: u32) {
    let texel = vec3<i32>(global_invocation_id);
    let constant = textureLoad(constants_map, texel).x;
    let prev_h = vec3<f32>(textureLoad(update_field_x, texel).x, textureLoad(update_field_y, texel).x, textureLoad(update_field_z, texel).x);
    let local_e_x = textureLoad(conjugative_field_x, texel).x;
    w_cache_x[local_invocation_index] = local_e_x;
    let local_e_y = textureLoad(conjugative_field_y, texel).x;
    w_cache_y[local_invocation_index] = local_e_y;
    let local_e_z = textureLoad(conjugative_field_z, texel).x;
    w_cache_z[local_invocation_index] = local_e_z;
    workgroupBarrier();
    let x_actual_texel = vec3<i32>(texel.x + 1, texel.y, texel.z);
    let x_cache_addr: u32 = local_invocation_index + 1u;
    var e_shift_x_y = 0.0;
    var e_shift_x_z = 0.0;
    if i32(local_invocation_id.x) < (WORKGROUP_X - 1) { // can't get rid of this branch since we actually want to reduce call to textureLoad :(
        e_shift_x_y = w_cache_y[x_cache_addr];
        e_shift_x_z = w_cache_z[x_cache_addr];
        } else {
        e_shift_x_y = textureLoad(conjugative_field_y, x_actual_texel).x;
        e_shift_x_z = textureLoad(conjugative_field_z, x_actual_texel).x;
    }
    var e_shift_y_x = 0.0;
    var e_shift_y_z = 0.0;
    let y_actual_texel = vec3<i32>(texel.x, texel.y + 1, texel.z);
    let y_cache_addr: u32 = local_invocation_index + u32(WORKGROUP_X);
    if i32(local_invocation_id.y) < (WORKGROUP_Y - 1) {
        e_shift_y_x = w_cache_x[y_cache_addr];
        e_shift_y_z = w_cache_z[y_cache_addr];
    } else {
        e_shift_y_x = textureLoad(conjugative_field_x, y_actual_texel).x;
        e_shift_y_z = textureLoad(conjugative_field_z, y_actual_texel).x;
    }
    var e_shift_z_x = 0.0;
    var e_shift_z_y = 0.0;
    let z_actual_texel = vec3<i32>(texel.x, texel.y, texel.z + 1);
    let z_cache_addr: u32 = local_invocation_index + u32(WORKGROUP_X) * u32(WORKGROUP_Y);
    if i32(local_invocation_id.z) < (WORKGROUP_Z - 1) {
        e_shift_z_x = w_cache_x[z_cache_addr];
        e_shift_z_y = w_cache_y[z_cache_addr];
    } else {
        e_shift_z_x = textureLoad(conjugative_field_x, z_actual_texel).x;
        e_shift_z_y = textureLoad(conjugative_field_y, z_actual_texel).x;
    }
    let diff_hx = (e_shift_z_y - local_e_y) - (e_shift_y_z - local_e_z);
    let diff_hy = (e_shift_x_z - local_e_z) - (e_shift_z_x - local_e_x);
    let diff_hz = (e_shift_y_x - local_e_x) - (e_shift_x_y - local_e_y);
    
    // PEC: no normal magnetic field
    var store_value = prev_h + constant * vec3<f32>(diff_hx, diff_hy, diff_hz) * vec3<f32>(f32((texel.x != i32(c_param.dimension.x) - 1) && texel.x != 0), f32((texel.y != i32(c_param.dimension.y) - 1) && texel.y != 0), f32((texel.z != i32(c_param.dimension.z) - 1) && texel.z != 0));
    textureStore(update_field_x, texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
    textureStore(update_field_y, texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}

@compute
@workgroup_size(WORKGROUP_X, WORKGROUP_Y, WORKGROUP_Z)
fn update_electric_field(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(local_invocation_id) local_invocation_id: vec3<u32>, @builtin(local_invocation_index) local_invocation_index: u32) {
    let texel = vec3<i32>(global_invocation_id);
    let constant = textureLoad(constants_map, texel).x;
    let prev_e = vec3<f32>(textureLoad(update_field_x, texel).x, textureLoad(update_field_y, texel).x, textureLoad(update_field_z, texel).x);
    let local_h_x = textureLoad(conjugative_field_x, texel).x;
    w_cache_x[local_invocation_index] = local_h_x;
    let local_h_y = textureLoad(conjugative_field_y, texel).x;
    w_cache_y[local_invocation_index] = local_h_y;
    let local_h_z = textureLoad(conjugative_field_z, texel).x;
    w_cache_z[local_invocation_index] = local_h_z;
    workgroupBarrier();
    var h_shift_x_y = 0.0;
    var h_shift_x_z = 0.0;
    let x_actual_texel = vec3<i32>(texel.x - 1, texel.y, texel.z);
    let x_cache_addr = local_invocation_index - 1u;
    if i32(local_invocation_id.x) > 0 {
        h_shift_x_y = w_cache_y[x_cache_addr];
        h_shift_x_z = w_cache_z[x_cache_addr];
    } else {
        h_shift_x_y = textureLoad(conjugative_field_y, x_actual_texel).x;
        h_shift_x_z = textureLoad(conjugative_field_z, x_actual_texel).x;
    }
    var h_shift_y_x = 0.0;
    var h_shift_y_z = 0.0;
    let y_actual_texel = vec3<i32>(texel.x, texel.y - 1, texel.z);
    let y_cache_addr: u32 = local_invocation_index - u32(WORKGROUP_X);
    if i32(local_invocation_id.y) > 0 {
        h_shift_y_x = w_cache_x[y_cache_addr];
        h_shift_y_z = w_cache_z[y_cache_addr];
    } else {
        h_shift_y_x = textureLoad(conjugative_field_x, y_actual_texel).x;
        h_shift_y_z = textureLoad(conjugative_field_z, y_actual_texel).x;
    }
    var h_shift_z_x = 0.0;
    var h_shift_z_y = 0.0;
    let z_actual_texel = vec3<i32>(texel.x, texel.y, texel.z - 1);
    let z_cache_addr: u32 = local_invocation_index - u32(WORKGROUP_X) * u32(WORKGROUP_Y);
    if i32(local_invocation_id.z) > 0 {
        h_shift_z_x = w_cache_x[z_cache_addr];
        h_shift_z_y = w_cache_y[z_cache_addr];
    } else {
        h_shift_z_x = textureLoad(conjugative_field_x, z_actual_texel).x;
        h_shift_z_y = textureLoad(conjugative_field_y, z_actual_texel).x;
    }
    let diff_ex = (local_h_z - h_shift_y_z) - (local_h_y - h_shift_z_y);
    let diff_ey = (local_h_x - h_shift_z_x) - (local_h_z - h_shift_x_z);
    let diff_ez = (local_h_y - h_shift_x_y) - (local_h_x - h_shift_y_x);

    // PEC: no tangential electric field
    var store_value = prev_e + constant * vec3<f32>(diff_ex, diff_ey, diff_ez) * vec3<f32>(
        f32((texel.y != i32(c_param.dimension.y) - 1) && (texel.y != 0) && (texel.z != i32(c_param.dimension.z) - 1) && (texel.z != 0)),
        f32((texel.x != i32(c_param.dimension.x) - 1) && (texel.x != 0) && (texel.z != i32(c_param.dimension.z) - 1) && (texel.z != 0)),
        f32((texel.x != i32(c_param.dimension.x) - 1) && (texel.x != 0) && (texel.y != i32(c_param.dimension.y) - 1) && (texel.y != 0)),
    );
    textureStore(update_field_x, texel, vec4<f32>(store_value.x, 0.0, 0.0, 1.0));
    textureStore(update_field_y, texel, vec4<f32>(store_value.y, 0.0, 0.0, 1.0));
    textureStore(update_field_z, texel, vec4<f32>(store_value.z, 0.0, 0.0, 1.0));
}

// REMOVE
const WORKGROUP_X: i32;
const WORKGROUP_Y: i32;
const WORKGROUP_Z: i32;
