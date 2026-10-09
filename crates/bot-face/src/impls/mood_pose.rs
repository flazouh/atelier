use std::f32::consts::PI;

use crate::enums::Mood;
use crate::structs::Pose;

/// The whole-body pose of a mood at time `t`. `ph` keeps a row of bots out of step.
pub(super) fn mood_pose(mood: Mood, t: f32, ph: f32) -> Pose {
    match mood {
        Mood::Idle => {
            let s = (t * 1.8 + ph).sin();
            let b = (t * 1.8 + ph + 0.6).sin();
            Pose::delta(0.0, -(1.0 + s) * 0.9, 0.0, -0.008 * b, 0.012 * b)
        }
        Mood::Thinking => Pose::delta(0.0, -1.0, 3.0 * (t * 1.4 + ph).sin(), 0.0, 0.0),
        Mood::Working => Pose::delta(
            0.9 * (t * 28.0).sin(),
            -0.5,
            0.0,
            0.0,
            0.01 * (t * 14.0).sin(),
        ),
        Mood::Done => hop(((t * 0.9 + ph / 6.3) % 1.0 + 1.0) % 1.0, 10.0),
        Mood::Needs => hop(((t * 1.6 + ph / 6.3) % 1.0 + 1.0) % 1.0, 7.0),
        Mood::Stuck => Pose::delta(
            0.8 * (t * 17.0).sin(),
            0.0,
            -8.0 + 1.5 * (t * 9.0).sin(),
            0.0,
            0.0,
        ),
    }
}

/// One hop over `u` from 0 to 1: up and down by `height`, a stretch in the air and a squash on landing.
fn hop(u: f32, height: f32) -> Pose {
    let k = u.min(1.0 - u);
    let squash = if k < 0.1 { 0.14 * (1.0 - k / 0.1) } else { 0.0 };
    let air = (PI * u).sin();
    let stretch = if squash > 0.0 { 0.0 } else { 0.07 * air };
    Pose::delta(
        0.0,
        -height * 4.0 * u * (1.0 - u),
        0.0,
        squash * 0.9 - 0.035 * air,
        -squash + stretch,
    )
}
