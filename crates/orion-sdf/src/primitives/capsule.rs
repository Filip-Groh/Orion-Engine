use glam::Vec3;
use crate::SDF;

pub struct SDFCapsule {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32
}

impl SDFCapsule {
    #[inline(always)]
    pub fn new(start: Vec3, end: Vec3, radius: f32) -> SDFCapsule {
        SDFCapsule {
            start,
            end,
            radius
        }
    }
}

impl SDF for SDFCapsule {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let pa = position - self.start;
        let ba = self.end - self.start;
        let h = (pa.dot(ba) / ba.length_squared()).clamp(0.0, 1.0);
        (pa - ba * h).length() - self.radius
    }
}