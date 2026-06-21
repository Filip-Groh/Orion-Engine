use bevy::math::Vec3;
use crate::constructive_solid_geometry::lerp;
use crate::SDF;

pub struct SDFSmoothSubtraction<A, B> {
    pub shape_a: A,
    pub shape_b: B,
    pub k: f32
}

impl<A, B> SDFSmoothSubtraction<A, B> {
    #[inline(always)]
    pub fn new(shape_a: A, shape_b: B, k: f32) -> SDFSmoothSubtraction<A, B> {
        SDFSmoothSubtraction {
            shape_a,
            shape_b,
            k
        }
    }
}

impl<A: SDF, B: SDF> SDF for SDFSmoothSubtraction<A, B> {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let d1 = self.shape_a.evaluate(position);
        let d2 = self.shape_b.evaluate(position);
        let h = (0.5 - 0.5 * (d2 + d1) / self.k).clamp(0.0, 1.0);

        lerp(d1, -d2, h) + self.k * h * (1.0 - h)
    }
}