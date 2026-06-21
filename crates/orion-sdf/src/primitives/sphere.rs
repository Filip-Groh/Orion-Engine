use bevy::math::Vec3;
use crate::SDF;

pub struct SDFSphere {
    pub center: Vec3,
    pub radius: f32
}

impl SDFSphere {
    #[inline(always)]
    pub fn new(center: Vec3, radius: f32) -> SDFSphere {
        SDFSphere {
            center,
            radius
        }
    }
}

impl SDF for SDFSphere {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let local_position = position - self.center;
        local_position.length() - self.radius
    }
}