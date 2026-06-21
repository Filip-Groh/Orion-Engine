use glam::Vec3;
use crate::constructive_solid_geometry::{SDFIntersection, SDFSmoothIntersection, SDFSmoothSubtraction, SDFSmoothUnion, SDFSubtraction, SDFUnion};

pub mod primitives;
pub mod constructive_solid_geometry;

pub trait SDF {
    fn evaluate(&self, position: Vec3) -> f32;

    fn union<S: SDF>(self, other: S) -> SDFUnion<Self, S> where Self: Sized {
        SDFUnion::new(self, other)
    }

    fn subtraction<S: SDF>(self, other: S) -> SDFSubtraction<Self, S> where Self: Sized {
        SDFSubtraction::new(self, other)
    }

    fn intersection<S: SDF>(self, other: S) -> SDFIntersection<Self, S> where Self: Sized {
        SDFIntersection::new(self, other)
    }

    fn smooth_union<S: SDF>(self, other: S, k: f32) -> SDFSmoothUnion<Self, S> where Self: Sized {
        SDFSmoothUnion::new(self, other, k)
    }

    fn smooth_subtraction<S: SDF>(self, other: S, k: f32) -> SDFSmoothSubtraction<Self, S> where Self: Sized {
        SDFSmoothSubtraction::new(self, other, k)
    }

    fn smooth_intersection<S: SDF>(self, other: S, k: f32) -> SDFSmoothIntersection<Self, S> where Self: Sized {
        SDFSmoothIntersection::new(self, other, k)
    }
}