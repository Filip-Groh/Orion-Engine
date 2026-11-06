struct Triangle {
    a: vec3<f32>,
    b: vec3<f32>,
    c: vec3<f32>,
}

struct Material {
    base_color: vec3<f32>,
    metallic: f32,
    roughness: f32,
}

struct Scene {
    eye: vec4<f32>,
    look_at: vec4<f32>,
    // Pixel width, pixel height, BGRA flag, and wireframe flags.
    extent_format: vec4<u32>,
    geometry_counts: vec4<u32>,
    // Spheres, triangles, BVH nodes, triangle UVs, texture descriptors, then texels.
    geometry: array<vec4<u32>>,
}

struct Surface {
    distance: f32,
    sphere_index: u32,
}

struct RayHit {
    distance: f32,
    triangle_index: u32,
    barycentric: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> scene: Scene;
@group(0) @binding(1) var<storage, read_write> pixels: array<u32>;

const HIT_EPSILON: f32 = 0.001;
const FAR: f32 = 50.0;
const NO_SPHERE: u32 = 0xffffffffu;
const NO_TRIANGLE: u32 = 0xffffffffu;
const MESH_WIREFRAME: u32 = 1u;
const BVH_WIREFRAME: u32 = 2u;

fn sample_scene(p: vec3<f32>) -> Surface {
    var closest = Surface(p.y, NO_SPHERE);
    for (var i = 0u; i < scene.geometry_counts.x; i++) {
        let offset = 2u * i;
        let center_radius = bitcast<vec4<f32>>(scene.geometry[offset]);
        let distance = length(p - center_radius.xyz) - center_radius.w;
        if distance < closest.distance {
            closest = Surface(distance, i);
        }
    }
    return closest;
}

fn material_at(offset: u32) -> Material {
    let packed = scene.geometry[offset];
    let factors = unpack2x16unorm(packed.w);
    return Material(bitcast<vec3<f32>>(packed.xyz), factors.x, factors.y);
}

fn sdf_material_at(p: vec3<f32>) -> Material {
    let index = sample_scene(p).sphere_index;
    if index != NO_SPHERE {
        return material_at(2u * index + 1u);
    }
    let tile = (i32(floor(p.x)) + i32(floor(p.z))) & 1;
    let gray = select(0.12, 0.21, tile == 0);
    return Material(vec3<f32>(gray), 0.0, 0.85);
}

fn march(origin: vec3<f32>, direction: vec3<f32>, limit: f32) -> f32 {
    var traveled = 0.0;
    for (var i = 0; i < 128 && traveled < limit; i++) {
        let step = abs(sample_scene(origin + direction * traveled).distance);
        if step < HIT_EPSILON {
            return traveled;
        }
        traveled += step;
    }
    return limit;
}
