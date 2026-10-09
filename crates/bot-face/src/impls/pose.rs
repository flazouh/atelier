use crate::structs::Pose;

impl Default for Pose {
    fn default() -> Self {
        Pose { x: 0.0, y: 0.0, r: 0.0, sx: 1.0, sy: 1.0 }
    }
}

impl Pose {
    /// A pose with a change of the given kinds. A scale of 0 here means no change, so `sx` and `sy` are offsets.
    pub fn delta(x: f32, y: f32, r: f32, dsx: f32, dsy: f32) -> Pose {
        Pose { x, y, r, sx: 1.0 + dsx, sy: 1.0 + dsy }
    }
}
