/// Where a part or a whole bot is: a move, a turn in degrees and a scale, all around a pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub sx: f32,
    pub sy: f32,
}
