use bevy::math::Vec3;
use crate::SDF;

pub struct SDFUnion<A, B> {
    pub shape_a: A,
    pub shape_b: B
}

impl<A, B> SDFUnion<A, B> {
    #[inline(always)]
    pub fn new(shape_a: A, shape_b: B) -> SDFUnion<A, B> {
        SDFUnion {
            shape_a,
            shape_b
        }
    }
}

impl<A: SDF, B: SDF> SDF for SDFUnion<A, B> {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        self.shape_a.evaluate(position).min(self.shape_b.evaluate(position))
    }
}