@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> tint: vec4<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generates a triangle covering the entire clip space (-1 to 3)
    let uv = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    let out_pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return VertexOutput(out_pos, vec2<f32>(uv.x, 1.0 - uv.y));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(screen_texture, texture_sampler, in.uv);

    let blended_rgb = mix(sampled.rgb, tint.rgb, tint.a);

    return vec4<f32>(blended_rgb, sampled.a);
}