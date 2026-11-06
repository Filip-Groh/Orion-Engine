//! Renderer-independent inputs for an SDF scene.

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

#[derive(Clone, Debug)]
pub struct Scene {
    pub camera: Camera,
    pub spheres: Vec<Sphere>,
}

impl Scene {
    pub fn demo() -> Self {
        Self {
            camera: Camera {
                eye: [3.4, 2.5, 5.0],
                target: [0.0, 0.7, 0.0],
                vertical_fov_degrees: 38.0,
            },
            spheres: vec![
                Sphere {
                    center: [-1.0, 0.95, 0.0],
                    radius: 0.95,
                    albedo: [0.90, 0.15, 0.06],
                },
                Sphere {
                    center: [0.92, 0.72, -0.3],
                    radius: 0.72,
                    albedo: [0.05, 0.42, 0.82],
                },
                Sphere {
                    center: [0.48, 0.34, 1.05],
                    radius: 0.34,
                    albedo: [0.94, 0.67, 0.12],
                },
            ],
        }
    }
}
