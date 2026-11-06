fn wrap_uv(uv: f32, mode: u32) -> f32 {
    if mode == 1u {
        return clamp(uv, 0.0, 1.0);
    }
    if mode == 2u {
        return 1.0 - abs(1.0 - 2.0 * fract(0.5 * uv));
    }
    return fract(uv);
}

fn wrap_texel(pixel: i32, size: i32, mode: u32) -> u32 {
    if mode == 0u {
        return u32((pixel % size + size) % size);
    }
    // Mirror UVs already reflect at the edge, so edge taps repeat the last texel.
    return u32(clamp(pixel, 0, size - 1));
}

fn texture_texel(info: vec4<u32>, pixel: vec2<i32>, modes: vec2<u32>, srgb: bool) -> vec3<f32> {
    let x = wrap_texel(pixel.x, i32(info.x), modes.x);
    let y = wrap_texel(pixel.y, i32(info.y), modes.y);
    let index = info.z + y * info.x + x;
    let rgb = unpack4x8unorm(scene.geometry[index / 4u][index % 4u]).rgb;
    if srgb {
        let low = rgb / 12.92;
        let high = pow((rgb + 0.055) / 1.055, vec3<f32>(2.4));
        return select(high, low, rgb <= vec3<f32>(0.04045));
    }
    return rgb;
}

fn sample_texture(offset: u32, uv: vec2<f32>, srgb: bool) -> vec3<f32> {
    let info = scene.geometry[offset];
    let transform = bitcast<vec4<f32>>(scene.geometry[offset + 1u]);
    let transformed = uv * transform.xy + transform.zw;
    if !all(abs(transformed) <= vec2<f32>(1e30)) {
        return vec3<f32>(1.0);
    }
    let modes = vec2<u32>(info.w & 3u, (info.w >> 2u) & 3u);
    let wrapped = vec2<f32>(wrap_uv(transformed.x, modes.x), wrap_uv(transformed.y, modes.y));
    let pixel = wrapped * vec2<f32>(info.xy);
    if (info.w & 16u) == 0u {
        return texture_texel(info, vec2<i32>(floor(pixel)), modes, srgb);
    }
    let centered = pixel - 0.5;
    let cell = vec2<i32>(floor(centered));
    let fraction = fract(centered);
    let a = texture_texel(info, cell, modes, srgb);
    let b = texture_texel(info, cell + vec2<i32>(1, 0), modes, srgb);
    let c = texture_texel(info, cell + vec2<i32>(0, 1), modes, srgb);
    let d = texture_texel(info, cell + vec2<i32>(1, 1), modes, srgb);
    return mix(mix(a, b, fraction.x), mix(c, d, fraction.x), fraction.y);
}

fn interpolate_uv(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, weights: vec3<f32>) -> vec2<f32> {
    return a * weights.x + b * weights.y + c * weights.z;
}

fn triangle_material(hit: RayHit) -> Material {
    let triangle = triangle_offset(hit.triangle_index);
    var material = material_at(triangle + 3u);
    let uv_index = scene.geometry[triangle].w;
    if uv_index == 0u {
        return material;
    }

    let offset = bvh_start() + 2u * scene.geometry_counts.z + 4u * (uv_index - 1u);
    let ab = bitcast<vec4<f32>>(scene.geometry[offset]);
    let cd = bitcast<vec4<f32>>(scene.geometry[offset + 1u]);
    let maps = scene.geometry[offset + 3u];
    let weights = vec3<f32>(1.0 - hit.barycentric.x - hit.barycentric.y, hit.barycentric);
    let primary = interpolate_uv(ab.xy, ab.zw, cd.xy, weights);
    if maps.x != 0u {
        material.base_color *= sample_texture(maps.x, primary, true);
    }
    if maps.y != 0u {
        let ef = bitcast<vec4<f32>>(scene.geometry[offset + 2u]);
        let secondary = interpolate_uv(cd.zw, ef.xy, ef.zw, weights);
        let factors = sample_texture(maps.y, secondary, false);
        material.roughness *= factors.g;
        material.metallic *= factors.b;
    }
    if maps.z != 0u {
        material.metallic *= sample_texture(maps.z, primary, false).r;
    }
    if maps.w != 0u {
        material.roughness *= sample_texture(maps.w, primary, false).r;
    }
    return material;
}
