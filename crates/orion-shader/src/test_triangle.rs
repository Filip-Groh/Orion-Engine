use spirv_std::glam::{Vec2, Vec3, Vec4, Mat4};
use spirv_std::spirv;
use crate::common::smoothstep;

// 1. Replicated Bevy View Bind Group layout (Set 0, Binding 0)
#[derive(Copy, Clone)]
#[repr(C)]
pub struct ViewUniform {
    pub view_projection: Mat4,
}

// 2. Exact layout of Bevy's Mesh Instancing Array element (Set 2, Binding 0)
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MeshUniform {
    pub model_to_world: Mat4,
    pub world_to_model: Mat4,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MaterialUniform {
    pub material_color: Vec4,
}

pub fn mesh_position_local_to_clip(
    view_proj: Mat4,
    model_to_world: Mat4,
    local_position: Vec3,
) -> Vec4 {
    let local_pos_4d = Vec4::new(local_position.x, local_position.y, local_position.z, 1.0);
    let world_pos = model_to_world * local_pos_4d;
    view_proj * world_pos
}

#[spirv(vertex(entry_point_name="vertex"))]
#[unsafe(no_mangle)]
pub fn vertex(
    #[spirv(instance_index)] instance_index: u32,
    #[spirv(location = 0)] position: Vec3,
    #[spirv(location = 1)] _normal: Vec3,
    #[spirv(location = 2)] uv: Vec2,

    // BINDINGS ALIGNED TO BEVY'S ENGINE:
    #[spirv(descriptor_set = 0, binding = 0, uniform)] view_data: &ViewUniform,
    #[spirv(descriptor_set = 2, binding = 0, storage_buffer)] mesh_instances: &[MeshUniform],

    #[spirv(position)] out_clip_position: &mut Vec4,
    #[spirv(location = 0)] out_uv: &mut Vec2,
) {
    let scaled_pos = position * 0.9;

    // Extract this instance's correct transform matrix from the storage array
    let model_to_world = mesh_instances[instance_index as usize].model_to_world;

    *out_clip_position = mesh_position_local_to_clip(
        view_data.view_projection,
        model_to_world,
        scaled_pos
    );

    *out_uv = uv;
}

#[spirv(fragment(entry_point_name="fragment"))]
#[unsafe(no_mangle)]
pub fn fragment(
    #[spirv(location = 0)] uv: Vec2,
    #[spirv(descriptor_set = 3, binding = 0, uniform)] material: &MaterialUniform,
    output_color: &mut Vec4,
) {
    let center_uv = uv - Vec2::new(0.5, 0.5);
    let dist_from_center = center_uv.length();

    let glow_intensity = smoothstep(0.2, 0.6, dist_from_center);
    let glow_color = Vec4::new(1.0, 0.4, 0.0, 1.0);

    *output_color = material.material_color.lerp(glow_color, glow_intensity);
}