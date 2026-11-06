//! ECS scene data for the SDF and triangle renderer.

mod demo;
mod glb;
mod obj;
mod snapshot;
mod texture;

use std::{fs, path::Path};

use anyhow::{Context, Result};
pub use hecs::{Entity, World};
use serde::{Deserialize, Serialize};
pub use snapshot::SceneSnapshot;
pub use texture::{MaterialMaps, Texture, TriangleUvs, WrapMode};

/// Perceptual roughness for imported geometry and older checkpoints.
pub const DEFAULT_ROUGHNESS: f32 = 0.55;

fn default_roughness() -> f32 {
    DEFAULT_ROUGHNESS
}

/// A perspective camera with +Y as world up.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub vertical_fov_degrees: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Sphere {
    pub center: [f32; 3],
    pub radius: f32,
    /// Linear RGB base color, with each component in `0.0..=1.0`.
    pub albedo: [f32; 3],
    /// Metallic fraction in `0.0..=1.0`.
    #[serde(default)]
    pub metallic: f32,
    /// Perceptual roughness in `0.0..=1.0`.
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

/// The rest height and vertical amplitude of a sine-wave animation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SineBob {
    pub rest_y: f32,
    pub amplitude: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Triangle {
    /// World-space vertices in counterclockwise order as seen from the front.
    pub vertices: [[f32; 3]; 3],
    /// Linear RGB base color, with each component in `0.0..=1.0`.
    pub albedo: [f32; 3],
    /// Metallic fraction in `0.0..=1.0`.
    #[serde(default)]
    pub metallic: f32,
    /// Perceptual roughness in `0.0..=1.0`.
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    /// Index of the triangle's UVs in its mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_index: Option<u32>,
}

/// One mesh object, represented by its world-space triangles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uvs: Vec<TriangleUvs>,
}

pub struct Scene {
    pub world: World,
    /// Texture images and samplers referenced by mesh material maps.
    pub textures: Vec<Texture>,
    /// The entity whose `Camera` component is used for rendering.
    pub active_camera: Entity,
}

impl Scene {
    pub fn new(camera: Camera) -> Self {
        let mut world = World::new();
        let active_camera = world.spawn((camera,));
        Self {
            world,
            textures: Vec::new(),
            active_camera,
        }
    }

    pub fn camera(&self) -> Result<Camera> {
        self.world
            .get::<&Camera>(self.active_camera)
            .map(|camera| *camera)
            .context("active camera entity must have a Camera component")
    }

    pub fn camera_mut(&mut self) -> Result<&mut Camera> {
        self.world
            .query_one_mut::<&mut Camera>(self.active_camera)
            .context("active camera entity must have a Camera component")
    }

    /// Spawns a Wavefront OBJ as one mesh entity in its original coordinates and winding.
    /// Faces use their MTL materials when present, the supplied fallback color, and flat normals.
    pub fn import_obj(&mut self, path: impl AsRef<Path>, albedo: [f32; 3]) -> Result<Entity> {
        let path = path.as_ref();
        let source =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let (mesh, textures) = obj::parse(&source, albedo, path.parent(), self.textures.len())
            .with_context(|| format!("import {}", path.display()))?;
        self.textures.extend(textures);
        Ok(self.world.spawn((mesh,)))
    }

    /// Spawns the static triangle geometry of a binary glTF asset as one mesh entity.
    /// Node transforms are baked into world-space positions. The supplied color is used for
    /// primitives without a material.
    pub fn import_glb(&mut self, path: impl AsRef<Path>, albedo: [f32; 3]) -> Result<Entity> {
        let path = path.as_ref();
        let source = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let (mesh, textures) = glb::parse(&source, albedo, path.parent(), self.textures.len())
            .with_context(|| format!("import {}", path.display()))?;
        self.textures.extend(textures);
        Ok(self.world.spawn((mesh,)))
    }
}
