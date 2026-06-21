mod union;
mod subtraction;
mod intersection;
mod smooth_union;
mod smooth_subtraction;
mod smooth_intersection;

pub use union::*;
pub use subtraction::*;
pub use intersection::*;
pub use smooth_union::*;
pub use smooth_subtraction::*;
pub use smooth_intersection::*;

#[inline(always)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}