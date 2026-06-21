use bevy::math::Vec3;
use crate::SDF;

pub struct SDFIntersection<A, B> {
    pub shape_a: A,
    pub shape_b: B
}

impl<A, B> SDFIntersection<A, B> {
    #[inline(always)]
    pub fn new(shape_a: A, shape_b: B) -> SDFIntersection<A, B> {
        SDFIntersection {
            shape_a,
            shape_b
        }
    }
}

impl<A: SDF, B: SDF> SDF for SDFIntersection<A, B> {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        self.shape_a.evaluate(position).max(self.shape_b.evaluate(position))
    }
}