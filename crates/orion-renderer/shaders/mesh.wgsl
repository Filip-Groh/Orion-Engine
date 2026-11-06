fn triangle_offset(index: u32) -> u32 {
    return 2u * scene.geometry_counts.x + 4u * index;
}

fn triangle_at(index: u32) -> Triangle {
    let offset = triangle_offset(index);
    return Triangle(bitcast<vec3<f32>>(scene.geometry[offset].xyz),
                    bitcast<vec3<f32>>(scene.geometry[offset + 1u].xyz),
                    bitcast<vec3<f32>>(scene.geometry[offset + 2u].xyz));
}

fn bvh_start() -> u32 {
    return triangle_offset(scene.geometry_counts.y);
}

// Front-face Möller–Trumbore intersection. FAR represents a miss.
fn triangle_intersection(origin: vec3<f32>, direction: vec3<f32>, triangle: Triangle) -> vec3<f32> {
    let ab = triangle.b - triangle.a;
    let ac = triangle.c - triangle.a;
    let p = cross(direction, ac);
    let determinant = dot(ab, p);
    if determinant <= 1e-6 * length(cross(ab, ac)) {
        return vec3<f32>(FAR, 0.0, 0.0);
    }

    let from_a = origin - triangle.a;
    let u = dot(from_a, p) / determinant;
    if u < 0.0 || u > 1.0 {
        return vec3<f32>(FAR, 0.0, 0.0);
    }
    let q = cross(from_a, ab);
    let v = dot(direction, q) / determinant;
    if v < 0.0 || u + v > 1.0 {
        return vec3<f32>(FAR, 0.0, 0.0);
    }
    return vec3<f32>(dot(ac, q) / determinant, u, v);
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
    var closest = RayHit(FAR, NO_TRIANGLE, vec2<f32>(0.0));
    let parallel = direction == vec3<f32>(0.0);
    let inverse_direction = 1.0 / select(direction, vec3<f32>(1.0), parallel);
    let nodes_start = bvh_start();
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
                let intersection = triangle_intersection(origin, direction, triangle_at(i));
                let distance = intersection.x;
                // Retain scene order when two triangles hit at the same distance.
                if distance > 0.0 && distance < FAR &&
                   (distance < closest.distance ||
                    (distance == closest.distance && i < closest.triangle_index)) {
                    closest = RayHit(distance, i, intersection.yz);
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
