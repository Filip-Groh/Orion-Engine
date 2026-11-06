use std::{
    collections::HashMap,
    f64::consts::{PI, TAU},
};

use anyhow::Result;
use orion_scene::{Camera, Entity, Scene, Sphere};

/// Transforms from an earlier simulation state. Geometry and materials stay in the scene.
#[derive(Default)]
pub struct TransformSnapshot {
    camera: Option<(Entity, Camera)>,
    spheres: HashMap<Entity, [f32; 3]>,
}

impl TransformSnapshot {
    pub fn capture(&mut self, scene: &Scene) -> Result<()> {
        self.camera = Some((scene.active_camera, scene.camera()?));
        self.spheres.clear();
        self.spheres.extend(
            scene
                .world
                .query::<(Entity, &Sphere)>()
                .iter()
                .map(|(entity, sphere)| (entity, sphere.center)),
        );
        Ok(())
    }

    pub fn sphere_center(&self, entity: Entity) -> Option<[f32; 3]> {
        self.spheres.get(&entity).copied()
    }
}

/// Optional edge overlays for a render frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wireframes {
    pub meshes: bool,
    pub bvh: bool,
}

/// Read-only scene sampling for a render frame.
pub struct SceneFrame<'a> {
    pub(super) scene: &'a Scene,
    pub(super) wireframes: Wireframes,
    previous: Option<(&'a TransformSnapshot, f64)>,
}

impl<'a> SceneFrame<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        Self {
            scene,
            wireframes: Wireframes::default(),
            previous: None,
        }
    }

    /// Samples from the previous transforms at fraction 0 to the scene's transforms at 1.
    pub fn interpolated(scene: &'a Scene, previous: &'a TransformSnapshot, fraction: f32) -> Self {
        Self {
            scene,
            wireframes: Wireframes::default(),
            previous: (fraction < 1.0).then_some((previous, f64::from(fraction.max(0.0)))),
        }
    }

    pub fn with_wireframes(mut self, wireframes: Wireframes) -> Self {
        self.wireframes = wireframes;
        self
    }

    pub(super) fn camera(&self) -> Result<Camera> {
        let current = self.scene.camera()?;
        if let Some((snapshot, fraction)) = self.previous
            && let Some((entity, previous)) = snapshot.camera
            && entity == self.scene.active_camera
        {
            return Ok(blend_camera(previous, current, fraction));
        }
        Ok(current)
    }

    pub(super) fn sphere_center(&self, entity: Entity, current: [f32; 3]) -> [f32; 3] {
        if let Some((snapshot, fraction)) = self.previous
            && let Some(previous) = snapshot.sphere_center(entity)
        {
            return blend_position(previous, current, fraction);
        }
        current
    }
}

fn blend_camera(previous: Camera, current: Camera, fraction: f64) -> Camera {
    if fraction == 0.0 {
        return previous;
    }

    let eye = blend_position(previous.eye, current.eye, fraction);
    let from = look_direction(previous);
    let to = look_direction(current);
    let target = if from == to {
        blend_position(previous.target, current.target, fraction)
    } else {
        // Blending opposite look vectors directly can make the camera look straight up or down.
        let start = from[2].atan2(from[0]);
        let end = to[2].atan2(to[0]);
        let turn = (end - start + PI).rem_euclid(TAU) - PI;
        let (sin, cos) = (start + turn * fraction).sin_cos();
        let horizontal = lerp(from[0].hypot(from[2]), to[0].hypot(to[2]), fraction);
        let direction = [
            horizontal * cos,
            lerp(from[1], to[1], fraction),
            horizontal * sin,
        ];
        std::array::from_fn(|i| (f64::from(eye[i]) + direction[i]) as f32)
    };
    Camera {
        eye,
        target,
        vertical_fov_degrees: lerp(
            f64::from(previous.vertical_fov_degrees),
            f64::from(current.vertical_fov_degrees),
            fraction,
        ) as f32,
    }
}

fn look_direction(camera: Camera) -> [f64; 3] {
    std::array::from_fn(|i| f64::from(camera.target[i]) - f64::from(camera.eye[i]))
}

fn blend_position(previous: [f32; 3], current: [f32; 3], fraction: f64) -> [f32; 3] {
    if fraction == 0.0 {
        return previous;
    }
    std::array::from_fn(|i| lerp(f64::from(previous[i]), f64::from(current[i]), fraction) as f32)
}

fn lerp(previous: f64, current: f64, fraction: f64) -> f64 {
    previous + (current - previous) * fraction
}
