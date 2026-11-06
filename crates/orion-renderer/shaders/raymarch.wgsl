struct Sphere {
    center_radius: vec4<f32>,
    albedo: vec4<f32>,
}

struct Scene {
    eye: vec4<f32>,
    look_at: vec4<f32>,
    extent_count: vec4<u32>,
    spheres: array<Sphere>,
}

struct Surface {
    distance: f32,
    albedo: vec3<f32>,
}

@group(0) @binding(0) var<storage, read> scene: Scene;
@group(0) @binding(1) var<storage, read_write> pixels: array<u32>;

const HIT_EPSILON: f32 = 0.001;
const FAR: f32 = 50.0;

fn sample_scene(p: vec3<f32>) -> Surface {
    let tile = (i32(floor(p.x)) + i32(floor(p.z))) & 1;
    let gray = select(0.12, 0.21, tile == 0);
    var closest = Surface(p.y, vec3<f32>(gray));
    for (var i = 0u; i < scene.extent_count.z; i++) {
        let sphere = scene.spheres[i];
        let distance = length(p - sphere.center_radius.xyz) - sphere.center_radius.w;
        if distance < closest.distance {
            closest = Surface(distance, sphere.albedo.xyz);
        }
    }
    return closest;
}

fn march(origin: vec3<f32>, direction: vec3<f32>) -> f32 {
    var traveled = 0.0;
    for (var i = 0; i < 128 && traveled < FAR; i++) {
        let step = abs(sample_scene(origin + direction * traveled).distance);
        if step < HIT_EPSILON {
            return traveled;
        }
        traveled += step;
    }
    return FAR;
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
    let distance = march(origin, direction);
    if distance >= FAR {
        return sky;
    }

    let p = origin + distance * direction;
    let normal = normal_at(p);
    let light = normalize(vec3<f32>(-0.6, 1.0, 0.45));
    var diffuse = max(dot(normal, light), 0.0);
    if diffuse > 0.0 && march(p + normal * (4.0 * HIT_EPSILON), light) < FAR {
        diffuse = 0.0;
    }
    let color = sample_scene(p).albedo * (0.24 + 0.76 * diffuse);
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
    let extent = scene.extent_count.xy;
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
    let ordered = select(color, color.bgr, scene.extent_count.w != 0u);
    pixels[id.y * extent.x + id.x] = pack4x8unorm(vec4<f32>(ordered, 1.0));
}
