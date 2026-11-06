//! ECS scene data for the SDF and triangle renderer.

mod demo;
mod obj;

use std::{fs, path::Path};

use anyhow::{Context, Result};
pub use hecs::{Entity, World};

/// A perspective camera with +Y as world up.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub vertical_fov_degrees: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub center: [f32; 3],
    pub radius: f32,
    /// Linear RGB reflectance, with each component in `0.0..=1.0`.
    pub albedo: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    /// World-space vertices in counterclockwise order as seen from the front.
    pub vertices: [[f32; 3]; 3],
    /// Linear RGB reflectance, with each component in `0.0..=1.0`.
    pub albedo: [f32; 3],
}

/// One mesh object, represented by its world-space triangles.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
}

pub struct Scene {
    pub world: World,
    /// The entity whose `Camera` component is used for rendering.
    pub active_camera: Entity,
}

impl Scene {
    pub fn new(camera: Camera) -> Self {
        let mut world = World::new();
        let active_camera = world.spawn((camera,));
        Self {
            world,
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
    /// The renderer uses the supplied albedo and computes flat normals from the faces.
    pub fn import_obj(&mut self, path: impl AsRef<Path>, albedo: [f32; 3]) -> Result<Entity> {
        let path = path.as_ref();
        let source =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let triangles =
            obj::parse(&source, albedo).with_context(|| format!("import {}", path.display()))?;
        Ok(self.world.spawn((Mesh { triangles },)))
    }
}
