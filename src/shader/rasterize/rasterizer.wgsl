struct Params {
    triangle_count: u32;
    layer: u32;
    permittivity: f32;
    permeability: f32;
    electric_conductivity: f32;
    magnetic_conductivity: f32;
    dt: f32;
    dx: f32;
};

struct IndicesBuffer {
    indices: array<u32>;
};

struct VerticesBuffer {
    vertices: array<vec3<f32>>;
};

struct Transform {
    transform: mat4x4<f32>;
};

var<push_constant> c_param: Params;

[[group(0), binding(0)]]
var<storage, read> indices_buffer: IndicesBuffer;

[[group(0), binding(1)]]
var<storage, read> vertices_buffer: VerticesBuffer;

[[group(0), binding(2)]]
var<uniform> u_transform: Transform;

[[group(0), binding(3)]]
var flag_map: texture_storage_3d<r32uint, read_write>;

[[group(1), binding(0)]]
var electric_constants_map: texture_storage_3d<rgba32float, write>;

[[group(1), binding(1)]]
var magnetic_constants_map: texture_storage_3d<rgba32float, write>;

[[group(2), binding(0)]]
var accumulator: texture_storage_3d<r32uint, read_write>;

[[stage(compute), workgroup_size(8, 8, 8)]]
fn predicate_ray_cross([[builtin(global_invocation_id)]] global_invocation_id: vec3<u32>) {
    if (global_invocation_id.x < c_param.triangle_count) {
        let v0 = (u_transform.transform * vec4<f32>(vertices_buffer.vertices[indices_buffer.indices[global_invocation_id.x * 3u] ], 1.0)).xyz;
        let v1 = (u_transform.transform * vec4<f32>(vertices_buffer.vertices[indices_buffer.indices[global_invocation_id.x * 3u + 1u] ], 1.0)).xyz;
        let v2 = (u_transform.transform * vec4<f32>(vertices_buffer.vertices[indices_buffer.indices[global_invocation_id.x * 3u + 2u] ], 1.0)).xyz;
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let ray = vec3<f32>(0.0, 0.0, 1.0);
        let p = vec3<f32>(f32(global_invocation_id.y), f32(global_invocation_id.z), 0.0);
        // let denominator = determinant(transpose(mat3x3<f32>(edge1, edge2, -ray)));
        // let nominator_u = determinant(transpose(mat3x3<f32>(p - v0, edge2, -ray)));
        // let nominator_v = determinant(transpose(mat3x3<f32>(edge1, p - v0, -ray)));
        // let nominator_t = determinant(transpose(mat3x3<f32>(edge1, edge2, p - v0)));
        let denominator = determinant(mat3x3<f32>(edge1, edge2, -ray));
        let nominator_u = determinant(mat3x3<f32>(p - v0, edge2, -ray));
        let nominator_v = determinant(mat3x3<f32>(edge1, p - v0, -ray));
        let nominator_t = determinant(mat3x3<f32>(edge1, edge2, p - v0));
        if (denominator != 0.0) {
            let u = nominator_u / denominator;
            let v = nominator_v / denominator;
            let t = nominator_t / denominator;
            if (u >= 0.0 && u <= 1.0 && v >= 0.0 && u + v <= 1.0 && t >= 0.0) {
                let h = p + ray * t;
                let round_h = vec3<i32>(floor(h));
                textureStore(flag_map, round_h, vec4<u32>(1u, 0u, 0u, 0u));
            }
        }
    }
}

[[stage(compute), workgroup_size(16, 16)]]
fn set_constants([[builtin(global_invocation_id)]] global_invocation_id: vec3<u32>) {
    let texel = vec3<i32>(i32(global_invocation_id.x), i32(global_invocation_id.y), i32(c_param.layer));
    var this_value = 0u;
    if (c_param.layer == 0u) {
        this_value = textureLoad(flag_map, texel).r;
    } else {
        let prev_layer = vec3<i32>(texel.x, texel.y, texel.z - 1);
        this_value = textureLoad(accumulator, prev_layer).r + textureLoad(flag_map, texel).r;
    }
    if (this_value % 2u == 1u) {
        let ec1 = (2.0 * c_param.permittivity - c_param.dt * c_param.electric_conductivity) / (2.0 * c_param.permittivity + c_param.dt * c_param.electric_conductivity);
        let ec3 = -2.0 * c_param.dt / (2.0 * c_param.permittivity + c_param.dt * c_param.electric_conductivity);
        let ec2 = ec3 / c_param.dx;
        let hc1 = (2.0 * c_param.permeability - c_param.dt * c_param.magnetic_conductivity) / (2.0 * c_param.permeability + c_param.dt * c_param.magnetic_conductivity);
        let hc3 = -2.0 * c_param.dt / (2.0 * c_param.permeability + c_param.dt * c_param.magnetic_conductivity);
        let hc2 = hc3 / c_param.dx;
        textureStore(electric_constants_map, texel, vec4<f32>(ec1, ec2, ec3, 1.0));
        textureStore(magnetic_constants_map, texel, vec4<f32>(hc1, hc2, hc3, 1.0));
    }
    textureStore(accumulator, texel, vec4<u32>(this_value, 0u, 0u, 1u));
}
