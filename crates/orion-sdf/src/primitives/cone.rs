use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use crate::SDF;

pub struct SDFCone { 
    pub angle_radians: f32, 
    pub height: f32 
}

impl SDFCone {
    #[inline(always)]
    pub fn new(angle_radians: f32, height: f32) -> SDFCone {
        SDFCone {
            angle_radians,
            height
        }
    }
}

impl SDF for SDFCone {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let (sin_a, cos_a) = self.angle_radians.sin_cos();
        let q = Vec2::new(position.xz().length(), position.y);
        let tip_dist = Vec2::new(q.x, q.y - self.height);
        let side_dist = q.dot(Vec2::new(cos_a, -sin_a));

        if tip_dist.y > 0.0 && tip_dist.x * sin_a + tip_dist.y * cos_a > 0.0 {
            tip_dist.length()
        } else {
            side_dist.max(position.y - self.height).max(-position.y)
        }
    }
}