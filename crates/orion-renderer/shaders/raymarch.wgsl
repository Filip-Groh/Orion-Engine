struct Triangle {
    a: vec3<f32>,
    b: vec3<f32>,
    c: vec3<f32>,
    albedo: vec3<f32>,
}

struct Scene {
    eye: vec4<f32>,
    look_at: vec4<f32>,
    extent_format: vec4<u32>,
    geometry_counts: vec4<u32>,
    // Two vec4s per sphere, four per triangle, then two per BVH node.
    geometry: array<vec4<u32>>,
}

struct Surface {
    distance: f32,
    albedo: vec3<f32>,
}

struct RayHit {
    distance: f32,
    triangle_index: u32,
}

@group(0) @binding(0) var<storage, read> scene: Scene;
@group(0) @binding(1) var<storage, read_write> pixels: array<u32>;

const HIT_EPSILON: f32 = 0.001;
const FAR: f32 = 50.0;
const NO_TRIANGLE: u32 = 0xffffffffu;

fn sample_scene(p: vec3<f32>) -> Surface {
    let tile = (i32(floor(p.x)) + i32(floor(p.z))) & 1;
    let gray = select(0.12, 0.21, tile == 0);
    var closest = Surface(p.y, vec3<f32>(gray));
    for (var i = 0u; i < scene.geometry_counts.x; i++) {
        let offset = 2u * i;
        let center_radius = bitcast<vec4<f32>>(scene.geometry[offset]);
        let distance = length(p - center_radius.xyz) - center_radius.w;
        if distance < closest.distance {
            closest = Surface(distance, bitcast<vec3<f32>>(scene.geometry[offset + 1u].xyz));
        }
    }
    return closest;
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

fn triangle_at(index: u32) -> Triangle {
    let offset = 2u * scene.geometry_counts.x + 4u * index;
    return Triangle(bitcast<vec3<f32>>(scene.geometry[offset].xyz),
                    bitcast<vec3<f32>>(scene.geometry[offset + 1u].xyz),
                    bitcast<vec3<f32>>(scene.geometry[offset + 2u].xyz),
                    bitcast<vec3<f32>>(scene.geometry[offset + 3u].xyz));
}

// Front-face Möller–Trumbore intersection. FAR represents a miss.
fn triangle_distance(origin: vec3<f32>, direction: vec3<f32>, triangle: Triangle) -> f32 {
    let ab = triangle.b - triangle.a;
    let ac = triangle.c - triangle.a;
    let p = cross(direction, ac);
    let determinant = dot(ab, p);
    if determinant <= 1e-6 * length(cross(ab, ac)) {
        return FAR;
    }

    let from_a = origin - triangle.a;
    let u = dot(from_a, p) / determinant;
    if u < 0.0 || u > 1.0 {
        return FAR;
    }
    let q = cross(from_a, ab);
    let v = dot(direction, q) / determinant;
    if v < 0.0 || u + v > 1.0 {
        return FAR;
    }
    return dot(ac, q) / determinant;
}

fn intersects_bounds(origin: vec3<f32>, inverse_direction: vec3<f32>, parallel: vec3<bool>,
                     lower: vec3<f32>, upper: vec3<f32>, limit: f32) -> bool {
    if any(parallel & (origin < lower)) || any(parallel & (origin > upper)) {
        return false;
    }
    let a = (lower - origin) * inverse_direction;
    let b = (upper - origin) * inverse_direction;
    let near = select(min(a, b), vec3<f32>(0.0), parallel);
    let far = select(max(a, b), vec3<f32>(limit), parallel);
    let enter = max(0.0, max(near.x, max(near.y, near.z)));
    let exit = min(limit, min(far.x, min(far.y, far.z)));
    // Keep the test inclusive for flat bounds and allow for rounding at the exit.
    return enter <= exit * 1.000001;
}

fn trace_meshes(origin: vec3<f32>, direction: vec3<f32>) -> RayHit {
    var closest = RayHit(FAR, NO_TRIANGLE);
    let parallel = direction == vec3<f32>(0.0);
    let inverse_direction = 1.0 / select(direction, vec3<f32>(1.0), parallel);
    let nodes_start = 2u * scene.geometry_counts.x + 4u * scene.geometry_counts.y;
    var node = 0u;
    while node < scene.geometry_counts.z {
        let offset = nodes_start + 2u * node;
        let lower = scene.geometry[offset];
        let upper = scene.geometry[offset + 1u];
        let first_or_escape = lower.w;
        let count = upper.w;
        if intersects_bounds(origin, inverse_direction, parallel, bitcast<vec3<f32>>(lower.xyz),
                             bitcast<vec3<f32>>(upper.xyz), closest.distance) {
            let end = first_or_escape + count;
            for (var i = first_or_escape; i < end; i++) {
                let distance = triangle_distance(origin, direction, triangle_at(i));
                // Retain scene order when two triangles hit at the same distance.
                if distance > 0.0 && distance < FAR &&
                   (distance < closest.distance ||
                    (distance == closest.distance && i < closest.triangle_index)) {
                    closest = RayHit(distance, i);
                }
            }
        } else if count == 0u {
            node = first_or_escape;
            continue;
        }
        node += 1u;
    }
    return closest;
}

fn trace_scene(origin: vec3<f32>, direction: vec3<f32>) -> RayHit {
    let closest = trace_meshes(origin, direction);
    let distance = march(origin, direction, closest.distance);
    if distance < closest.distance {
        return RayHit(distance, NO_TRIANGLE);
    }
    return closest;
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(HIT_EPSILON, 0.0);
    return normalize(vec3<f32>(
        sample_scene(p + e.xyy).distance - sample_scene(p - e.xyy).distance,
        sample_scene(p + e.yxy).distance - sample_scene(p - e.yxy).distance,
        sample_scene(p + e.yyx).distance - sample_scene(p - e.yyx).distance,
    ));
}

fn shade(origin: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    let sky = mix(vec3<f32>(0.37, 0.51, 0.68), vec3<f32>(0.10, 0.22, 0.41),
                  clamp(direction.y * 2.0, 0.0, 1.0));
    let hit = trace_scene(origin, direction);
    let distance = hit.distance;
    if distance >= FAR {
        return sky;
    }

    let p = origin + distance * direction;
    var normal: vec3<f32>;
    var albedo: vec3<f32>;
    if hit.triangle_index == NO_TRIANGLE {
        normal = normal_at(p);
        albedo = sample_scene(p).albedo;
    } else {
        let triangle = triangle_at(hit.triangle_index);
        normal = normalize(cross(triangle.b - triangle.a, triangle.c - triangle.a));
        albedo = triangle.albedo;
    }
    let light = normalize(vec3<f32>(-0.6, 1.0, 0.45));
    var diffuse = max(dot(normal, light), 0.0);
    if diffuse > 0.0 && trace_scene(p + normal * (4.0 * HIT_EPSILON), light).distance < FAR {
        diffuse = 0.0;
    }
    let color = albedo * (0.24 + 0.76 * diffuse);
    return mix(color, sky, 1.0 - exp(-0.0018 * distance * distance));
}

fn to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

// Keep the workgroup size in sync with WORKGROUP_SIZE in the renderer.
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let extent = scene.extent_format.xy;
    if any(id.xy >= extent) {
        return;
    }

    let size = vec2<f32>(extent);
    let screen = (2.0 * (vec2<f32>(id.xy) + 0.5) - size) / size.y;
    let forward = normalize(scene.look_at.xyz - scene.eye.xyz);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let direction = normalize(forward + scene.eye.w * (screen.x * right - screen.y * up));
    let color = to_srgb(shade(scene.eye.xyz, direction));
    let ordered = select(color, color.bgr, scene.extent_format.z != 0u);
    pixels[id.y * extent.x + id.x] = pack4x8unorm(vec4<f32>(ordered, 1.0));
}
