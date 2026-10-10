use super::Pose;

/// Everything the painter needs to draw one bot at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub root: Pose,
    /// One pose for each part, in the order of the bot's parts.
    pub parts: Vec<Pose>,
    /// How much each mood's eyes show, in the order of `Mood::ALL`.
    pub eye_weights: [f32; 6],
    /// The height the idle and the needs eyes keep while the bot blinks: 1 open, near 0 shut.
    pub blink: f32,
    pub scan_x: f32,
    pub look: (f32, f32),
    pub cheeks: f32,
}
