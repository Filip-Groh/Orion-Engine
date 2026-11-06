use anyhow::{Result, ensure};
use ash::vk;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use orion_scene::{Camera, Entity, Mesh, Scene, Sphere, Triangle};

use crate::{
    SceneFrame,
    bvh::{self, BvhNode},
};

/// Geometry totals for the renderer's scene overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStats {
    pub triangles: usize,
    pub sdfs: usize,
    pub bvh_nodes: usize,
}

impl SceneStats {
    pub fn from_scene(scene: &Scene) -> Self {
        let spheres = scene.world.query::<&Sphere>().iter().len();
        let (mesh_count, triangles) = mesh_counts(scene.world.query::<&Mesh>().iter());
        Self {
            triangles,
            // sample_scene in scene.wgsl also includes the implicit ground plane.
            sdfs: spheres + 1,
            bvh_nodes: bvh::node_count(mesh_count),
        }
    }
}

// The WGSL geometry array follows the header in sphere, triangle, BVH node order.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneHeader {
    eye_and_fov_scale: [f32; 4],
    look_at: [f32; 4],
    extent_format: [u32; 4],
    geometry_counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuSphere {
    center_radius: [f32; 4],
    albedo: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuTriangle {
    vertices: [[f32; 4]; 3],
    albedo: [f32; 4],
}

pub(super) fn pack_scene(
    frame: SceneFrame<'_>,
    extent: vk::Extent2D,
    bgra: bool,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let mut header = SceneHeader::new(frame.camera()?, extent, bgra).or_else(|_| {
        // At large coordinates, rounding an interpolated target can collapse it onto the eye.
        SceneHeader::new(frame.scene.camera()?, extent, bgra)
    })?;
    // Keep these bits in sync with the flags in scene.wgsl.
    header.extent_format[3] =
        u32::from(frame.wireframes.meshes) | (u32::from(frame.wireframes.bvh) << 1);
    let mut spheres = frame.scene.world.query::<(Entity, &Sphere)>();
    let mut meshes = frame.scene.world.query::<&Mesh>();
    let sphere_count = spheres.iter().len();
    let (mesh_count, triangle_count) = mesh_counts(meshes.iter());
    let node_count = bvh::node_count(mesh_count);
    let size = scene_buffer_size(sphere_count, triangle_count, node_count, max_bytes)?;
    header.geometry_counts = [
        sphere_count as u32,
        triangle_count as u32,
        node_count as u32,
        0,
    ];

    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(bytes_of(&header));
    for (entity, sphere) in spheres.iter() {
        let sphere = Sphere {
            center: frame.sphere_center(entity, sphere.center),
            ..*sphere
        };
        bytes.extend_from_slice(bytes_of(&GpuSphere::try_from(&sphere)?));
    }
    let mut leaves = Vec::with_capacity(mesh_count);
    let mut first_triangle = 0;
    for mesh in meshes.iter().filter(|mesh| !mesh.triangles.is_empty()) {
        leaves.push(append_mesh(&mut bytes, mesh, first_triangle)?);
        first_triangle += mesh.triangles.len() as u32;
    }
    bytes.extend_from_slice(cast_slice(&bvh::build(&mut leaves)));
    bytes.resize(size, 0);
    Ok(bytes)
}

fn mesh_counts<'a>(meshes: impl Iterator<Item = &'a Mesh>) -> (usize, usize) {
    meshes
        .map(|mesh| mesh.triangles.len())
        .filter(|&count| count > 0)
        .fold((0, 0), |(meshes, triangles), count| {
            (meshes + 1, triangles + count)
        })
}

impl SceneHeader {
    fn new(camera: Camera, extent: vk::Extent2D, bgra: bool) -> Result<Self> {
        let [x, y, z] = camera.eye;
        let [tx, ty, tz] = camera.target;
        let horizontal_squared = (tx - x).powi(2) + (tz - z).powi(2);
        let distance_squared = horizontal_squared + (ty - y).powi(2);
        ensure!(
            horizontal_squared >= f32::MIN_POSITIVE && distance_squared.is_finite(),
            "camera must look in a finite direction with a horizontal component"
        );
        let fov = camera.vertical_fov_degrees;
        ensure!(
            fov > 0.0 && fov < 180.0,
            "vertical FOV must be between 0 and 180 degrees"
        );
        Ok(Self {
            eye_and_fov_scale: [x, y, z, (0.5 * fov.to_radians()).tan()],
            look_at: [tx, ty, tz, 0.0],
            extent_format: [extent.width, extent.height, u32::from(bgra), 0],
            geometry_counts: [0; 4],
        })
    }
}

fn scene_buffer_size(
    spheres: usize,
    triangles: usize,
    nodes: usize,
    max_bytes: usize,
) -> Result<usize> {
    // The runtime array needs one vec4 even when the scene has no geometry.
    let geometry_bytes = spheres
        .saturating_mul(size_of::<GpuSphere>())
        .saturating_add(triangles.saturating_mul(size_of::<GpuTriangle>()))
        .saturating_add(nodes.saturating_mul(size_of::<BvhNode>()))
        .max(size_of::<[f32; 4]>());
    let header_bytes = size_of::<SceneHeader>();
    ensure!(
        geometry_bytes <= max_bytes.saturating_sub(header_bytes),
        "scene exceeds the device's storage buffer limit"
    );
    Ok(header_bytes + geometry_bytes)
}

impl TryFrom<&Sphere> for GpuSphere {
    type Error = anyhow::Error;

    fn try_from(sphere: &Sphere) -> Result<Self> {
        ensure!(
            sphere.radius.is_finite()
                && sphere.radius > 0.0
                && sphere
                    .center
                    .iter()
                    .all(|coordinate| coordinate.is_finite())
                && valid_albedo(sphere.albedo),
            "spheres require a finite center, a positive finite radius, and albedo in 0..=1"
        );
        let [x, y, z] = sphere.center;
        Ok(Self {
            center_radius: [x, y, z, sphere.radius],
            albedo: padded(sphere.albedo),
        })
    }
}

impl TryFrom<&Triangle> for GpuTriangle {
    type Error = anyhow::Error;

    fn try_from(triangle: &Triangle) -> Result<Self> {
        ensure!(
            triangle
                .vertices
                .iter()
                .flatten()
                .all(|coordinate| coordinate.is_finite())
                && valid_albedo(triangle.albedo),
            "triangles require finite vertices and albedo in 0..=1"
        );
        Ok(Self {
            vertices: triangle.vertices.map(padded),
            albedo: padded(triangle.albedo),
        })
    }
}

fn append_mesh(bytes: &mut Vec<u8>, mesh: &Mesh, first_triangle: u32) -> Result<BvhNode> {
    let mut leaf = BvhNode::new(first_triangle, mesh.triangles.len() as u32);
    for triangle in &mesh.triangles {
        let gpu_triangle = GpuTriangle::try_from(triangle)?;
        for vertex in triangle.vertices {
            leaf.include(vertex);
        }
        bytes.extend_from_slice(bytes_of(&gpu_triangle));
    }
    Ok(leaf)
}

fn valid_albedo(albedo: [f32; 3]) -> bool {
    albedo.into_iter().all(|color| (0.0..=1.0).contains(&color))
}

fn padded([x, y, z]: [f32; 3]) -> [f32; 4] {
    [x, y, z, 0.0]
}

#[cfg(test)]
mod tests;
