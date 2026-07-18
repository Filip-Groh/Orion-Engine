#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}

@group(3) @binding(0) var<uniform> material_color: vec4<f32>;

struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        vec4<f32>(vertex.position * 0.9, 1.0)
    );

    out.uv = vertex.uv;

    return out;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let center_uv = mesh.uv - vec2<f32>(0.5, 0.5);

    let dist_from_center = length(center_uv);
    
    let glow_intensity = smoothstep(0.2, 0.6, dist_from_center);
    let glow_color = vec4<f32>(1.0, 0.4, 0.0, 1.0);

    return mix(material_color, glow_color, glow_intensity);
}