use glam::{Vec2, Vec3, Vec3Swizzles};
use crate::SDF;

pub struct SDFCylinder {
    pub radius: f32,
    pub height: f32
}

impl SDFCylinder {
    #[inline(always)]
    pub fn new(radius: f32, height: f32) -> SDFCylinder {
        SDFCylinder { 
            radius, 
            height 
        }
    }
}

impl SDF for SDFCylinder {
    #[inline(always)]
    fn evaluate(&self, position: Vec3) -> f32 {
        let d = Vec2::new(position.xz().length(), position.y).abs() - Vec2::new(self.radius, self.height);
        let ext = d.max(Vec2::ZERO).length();
        let m_el = d.max_element();
        let int = if m_el < 0.0 { m_el } else { 0.0 };
        ext + int
    }
}