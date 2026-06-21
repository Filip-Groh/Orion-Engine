use glam::Vec3;
use crate::SDF;

pub struct SDFPlane {
    pub normal: Vec3,
    pub height: f32
}

impl SDFPlane {
    #[inline(always)]
    pub fn new(normal: Vec3, height: f32) -> SDFPlane {
        SDFPlane {
            normal,
            height
        }
    }
}

impl SDF for SDFPlane {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        position.dot(self.normal.normalize()) + self.height
    }
}