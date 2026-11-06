// Find the point on a segment closest to the ray within the visible depth range.
// The radius scales with depth to keep distant lines visible.
fn wire_segment_distance(origin: vec3<f32>, direction: vec3<f32>, a: vec3<f32>,
                         b: vec3<f32>, radius_per_depth: f32, limit: f32) -> f32 {
    let from_eye = a - origin;
    let segment = b - a;
    let depth_a = dot(from_eye, direction);
    let depth_step = dot(segment, direction);
    let perpendicular_a = from_eye - depth_a * direction;
    let perpendicular_step = segment - depth_step * direction;
    var start = 0.0;
    var end = 1.0;
    if abs(depth_step) > 1e-6 {
        let near = (HIT_EPSILON - depth_a) / depth_step;
        let far = (limit - depth_a) / depth_step;
        start = max(start, min(near, far));
        end = min(end, max(near, far));
        if start > end {
            return FAR;
        }
    }
    let t = clamp(-dot(perpendicular_a, perpendicular_step) /
                  max(dot(perpendicular_step, perpendicular_step), 1e-20), start, end);
    let depth = depth_a + t * depth_step;
    let offset = perpendicular_a + t * perpendicular_step;
    let radius = radius_per_depth * depth;
    return select(FAR, depth, depth > 0.0 && depth <= limit &&
                  dot(offset, offset) <= radius * radius);
}

fn mesh_wireframe(origin: vec3<f32>, direction: vec3<f32>, triangle: Triangle,
                  radius_per_depth: f32) -> bool {
    return wire_segment_distance(origin, direction, triangle.a, triangle.b, radius_per_depth, FAR) < FAR ||
           wire_segment_distance(origin, direction, triangle.b, triangle.c, radius_per_depth, FAR) < FAR ||
           wire_segment_distance(origin, direction, triangle.c, triangle.a, radius_per_depth, FAR) < FAR;
}

fn box_wireframe(origin: vec3<f32>, direction: vec3<f32>, lower: vec3<f32>, upper: vec3<f32>,
                 radius_per_depth: f32, limit: f32) -> bool {
    // Four edges parallel to each axis.
    for (var axis = 0u; axis < 3u; axis++) {
        let next = (axis + 1u) % 3u;
        let last = (axis + 2u) % 3u;
        for (var corner = 0u; corner < 4u; corner++) {
            var a = lower;
            a[next] = select(lower[next], upper[next], (corner & 1u) != 0u);
            a[last] = select(lower[last], upper[last], (corner & 2u) != 0u);
            var b = a;
            b[axis] = upper[axis];
            if wire_segment_distance(origin, direction, a, b, radius_per_depth, limit) < FAR {
                return true;
            }
        }
    }
    return false;
}

fn bvh_wireframe(origin: vec3<f32>, direction: vec3<f32>, radius_per_depth: f32,
                 limit: f32) -> bool {
    let parallel = direction == vec3<f32>(0.0);
    let inverse_direction = 1.0 / select(direction, vec3<f32>(1.0), parallel);
    let nodes_start = bvh_start();
    let padding = vec3<f32>(radius_per_depth * limit);
    var node = 0u;
    while node < scene.geometry_counts.z {
        let offset = nodes_start + 2u * node;
        let lower = scene.geometry[offset];
        let upper = scene.geometry[offset + 1u];
        let min_corner = bitcast<vec3<f32>>(lower.xyz);
        let max_corner = bitcast<vec3<f32>>(upper.xyz);
        if intersects_bounds(origin, inverse_direction, parallel,
                             min_corner - padding, max_corner + padding, limit) {
            if box_wireframe(origin, direction, min_corner, max_corner, radius_per_depth, limit) {
                return true;
            }
        } else if upper.w == 0u {
            node = lower.w;
            continue;
        }
        node += 1u;
    }
    return false;
}
