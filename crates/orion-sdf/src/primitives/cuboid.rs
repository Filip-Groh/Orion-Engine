use glam::Vec3;
use crate::SDF;

pub struct SDFCuboid {
    pub bounds: Vec3
}

impl SDFCuboid {
    #[inline(always)]
    pub fn new(bounds: Vec3) -> SDFCuboid {
        SDFCuboid { 
            bounds
        }
    }
}

impl SDF for SDFCuboid {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let d = position.abs() - self.bounds;
        let ext = d.max(Vec3::ZERO).length();
        let m_el = d.max_element();
        let int = if m_el < 0.0 { m_el } else { 0.0 };
        ext + int
    }
}