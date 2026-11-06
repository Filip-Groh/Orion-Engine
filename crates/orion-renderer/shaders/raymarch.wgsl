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

fn shade(origin: vec3<f32>, direction: vec3<f32>, pixel_scale: f32) -> vec3<f32> {
    let sky = mix(vec3<f32>(0.37, 0.51, 0.68), vec3<f32>(0.10, 0.22, 0.41),
                  clamp(direction.y * 2.0, 0.0, 1.0));
    let hit = trace_scene(origin, direction);
    let distance = hit.distance;
    let wireframes = scene.extent_format.w;
    if (wireframes & BVH_WIREFRAME) != 0u &&
       bvh_wireframe(origin, direction, pixel_scale, min(FAR, distance + HIT_EPSILON)) {
        return vec3<f32>(1.0, 0.36, 0.025);
    }
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
        if (wireframes & MESH_WIREFRAME) != 0u &&
           mesh_wireframe(origin, direction, triangle, pixel_scale * 1.4) {
            return vec3<f32>(0.04, 0.85, 1.0);
        }
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
    let pixel_scale = 2.0 * scene.eye.w * dot(direction, forward) / size.y;
    let color = to_srgb(shade(scene.eye.xyz, direction, pixel_scale));
    let ordered = select(color, color.bgr, scene.extent_format.z != 0u);
    pixels[id.y * extent.x + id.x] = pack4x8unorm(vec4<f32>(ordered, 1.0));
}
