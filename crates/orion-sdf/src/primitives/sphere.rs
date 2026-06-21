use bevy::math::Vec3;
use crate::SDF;

pub struct SDFSphere { 
    pub radius: f32 
}

impl SDFSphere {
    #[inline(always)]
    pub fn new(radius: f32) -> SDFSphere {
        SDFSphere { 
            radius 
        }
    }
}

impl SDF for SDFSphere {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        position.length() - self.radius
    }
}