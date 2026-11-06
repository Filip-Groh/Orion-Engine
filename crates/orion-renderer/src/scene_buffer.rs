use anyhow::{Result, ensure};
use ash::vk;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use orion_scene::{Camera, Entity, MaterialMaps, Mesh, Scene, Sphere, Triangle};

mod textures;
use textures::{GpuUvs, TextureLayout};

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

// Spheres, triangles, BVH nodes, triangle UVs, texture descriptors, then texture texels.
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
    material: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuTriangle {
    vertices: [[u32; 4]; 3],
    material: [u32; 4],
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
    let uv_count = meshes
        .iter()
        .filter(|mesh| !mesh.triangles.is_empty())
        .flat_map(|mesh| &mesh.uvs)
        .filter(|uv| uv.maps != MaterialMaps::default())
        .count();
    let geometry_bytes = sphere_count
        .saturating_mul(size_of::<GpuSphere>())
        .saturating_add(triangle_count.saturating_mul(size_of::<GpuTriangle>()))
        .saturating_add(node_count.saturating_mul(size_of::<BvhNode>()));
    let texture_start = geometry_bytes.saturating_add(uv_count.saturating_mul(size_of::<GpuUvs>()));
    let textures = TextureLayout::new(&frame.scene.textures, texture_start)?;
    // Pad to a whole vec4 and keep a single slot even when the scene is empty.
    let payload_size = textures.end.saturating_add(15) / 16 * 16;
    let size = size_of::<SceneHeader>().saturating_add(payload_size.max(16));
    ensure!(
        size <= max_bytes,
        "scene exceeds the device's storage buffer limit"
    );
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
    let mut packed_uvs = Vec::with_capacity(uv_count);
    let mut first_triangle = 0;
    for mesh in meshes.iter().filter(|mesh| !mesh.triangles.is_empty()) {
        let uv_indices = mesh
            .uvs
            .iter()
            .map(|&uv| -> Result<u32> {
                if let Some(uv) = textures.pack_uvs(uv)? {
                    packed_uvs.push(uv);
                    return Ok(packed_uvs.len().try_into()?);
                }
                Ok(0)
            })
            .collect::<Result<Vec<_>>>()?;
        leaves.push(append_mesh(&mut bytes, mesh, first_triangle, &uv_indices)?);
        first_triangle += mesh.triangles.len() as u32;
    }
    bytes.extend_from_slice(cast_slice(&bvh::build(&mut leaves)));
    bytes.extend_from_slice(cast_slice(&packed_uvs));
    textures.append(&mut bytes)?;
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

impl TryFrom<&Sphere> for GpuSphere {
    type Error = anyhow::Error;

    fn try_from(sphere: &Sphere) -> Result<Self> {
        ensure!(
            sphere.radius.is_finite()
                && sphere.radius > 0.0
                && sphere
                    .center
                    .iter()
                    .all(|coordinate| coordinate.is_finite()),
            "spheres require a finite center and a positive finite radius"
        );
        let [x, y, z] = sphere.center;
        Ok(Self {
            center_radius: [x, y, z, sphere.radius],
            material: pack_material(sphere.albedo, sphere.metallic, sphere.roughness)?,
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
                .all(|coordinate| coordinate.is_finite()),
            "triangles require finite vertices"
        );
        Ok(Self {
            vertices: triangle
                .vertices
                .map(|[x, y, z]| [x.to_bits(), y.to_bits(), z.to_bits(), 0]),
            material: pack_material(triangle.albedo, triangle.metallic, triangle.roughness)?,
        })
    }
}

fn append_mesh(
    bytes: &mut Vec<u8>,
    mesh: &Mesh,
    first_triangle: u32,
    uv_indices: &[u32],
) -> Result<BvhNode> {
    let mut leaf = BvhNode::new(first_triangle, mesh.triangles.len() as u32);
    for triangle in &mesh.triangles {
        let mut gpu_triangle = GpuTriangle::try_from(triangle)?;
        if let Some(index) = triangle.uv_index {
            gpu_triangle.vertices[0][3] = *uv_indices
                .get(index as usize)
                .ok_or_else(|| anyhow::anyhow!("triangle UV index {index} is out of bounds"))?;
        }
        for vertex in triangle.vertices {
            leaf.include(vertex);
        }
        bytes.extend_from_slice(bytes_of(&gpu_triangle));
    }
    Ok(leaf)
}

fn pack_material(base_color: [f32; 3], metallic: f32, roughness: f32) -> Result<[u32; 4]> {
    let [r, g, b] = base_color;
    ensure!(
        [r, g, b, metallic, roughness]
            .into_iter()
            .all(|value| (0.0..=1.0).contains(&value)),
        "material base color, metallic, and roughness must be in 0..=1"
    );
    // The shader reads the last word with unpack2x16unorm.
    let unorm16 = |value: f32| (value * f32::from(u16::MAX)).round() as u32;
    let factors = unorm16(metallic) | (unorm16(roughness) << 16);
    Ok([r.to_bits(), g.to_bits(), b.to_bits(), factors])
}

#[cfg(test)]
mod tests;
