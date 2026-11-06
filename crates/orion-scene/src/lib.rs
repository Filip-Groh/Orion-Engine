//! ECS scene data for the SDF and triangle renderer.

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

    pub fn demo() -> Self {
        let mut suzanne = obj::parse(include_str!("../assets/monkey.obj"), [0.68, 0.36, 0.12])
            .expect("bundled Suzanne OBJ is valid");
        for triangle in &mut suzanne {
            triangle.vertices = triangle
                .vertices
                .map(|[x, y, z]| [0.75 * x + 1.85, 0.75 * y + 0.85, 0.75 * z - 1.25]);
        }

        let mut scene = Self::new(Camera {
            eye: [3.4, 2.5, 5.0],
            target: [0.0, 0.7, 0.0],
            vertical_fov_degrees: 38.0,
        });
        scene.world.spawn_batch([
            (Sphere {
                center: [-1.0, 0.95, 0.0],
                radius: 0.95,
                albedo: [0.90, 0.15, 0.06],
            },),
            (Sphere {
                center: [0.92, 0.72, -0.3],
                radius: 0.72,
                albedo: [0.05, 0.42, 0.82],
            },),
            (Sphere {
                center: [0.48, 0.34, 1.05],
                radius: 0.34,
                albedo: [0.94, 0.67, 0.12],
            },),
        ]);
        scene.world.spawn_batch([
            (Mesh {
                triangles: vec![Triangle {
                    // This triangle crosses the red sphere and extends past its silhouette.
                    vertices: [[-2.35, 0.30, 0.50], [0.25, 0.50, 0.50], [-0.95, 2.28, 0.50]],
                    albedo: [0.12, 0.76, 0.38],
                }],
            },),
            (Mesh { triangles: suzanne },),
        ]);
        scene
    }
}
