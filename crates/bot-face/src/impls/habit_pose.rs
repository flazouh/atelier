use std::f32::consts::PI;

use crate::enums::HabitKind;
use crate::structs::{HabitDef, Pose};

/// The pose of one habit at time `t`. `speed` and `amount` come from the moods, mixed by weight.
pub(super) fn habit_pose(
    h: &HabitDef,
    pivot: (f32, f32),
    t: f32,
    speed: f32,
    amount: f32,
    ph: f32,
) -> Pose {
    let hz = h.hz.unwrap_or(1.0);
    let wave = |k: f32| (t * speed * hz * k + h.phase.unwrap_or(0.0)).sin();
    match h.kind {
        HabitKind::Swing => match h.every_s {
            Some(every) => Pose::delta(
                0.0,
                0.0,
                -amount * h.amp_deg.unwrap_or(0.0) * taps(t, speed, every),
                0.0,
                0.0,
            ),
            None => Pose::delta(
                0.0,
                0.0,
                amount * h.amp_deg.unwrap_or(0.0) * wave(1.0),
                0.0,
                0.0,
            ),
        },
        HabitKind::Spin => Pose::delta(
            0.0,
            0.0,
            t * (speed - 1.2).max(0.0) * h.deg_per_s.unwrap_or(0.0),
            0.0,
            0.0,
        ),
        HabitKind::Sweep => Pose::delta(
            amount * h.dx.unwrap_or(0.0) * wave(1.0),
            0.0,
            amount * h.rot_deg.unwrap_or(0.0) * (t * speed * hz + 1.0).sin(),
            0.0,
            0.0,
        ),
        HabitKind::Spring => {
            let s = (t * speed * hz + ph).sin();
            let squash = h.squash.unwrap_or(0.0);
            Pose::delta(
                0.0,
                0.0,
                0.0,
                -amount * squash * 0.43 * s,
                amount * squash * s,
            )
        }
        HabitKind::Flicker => {
            let o = pivot.0 * 0.05;
            let amp = h.amp.unwrap_or(0.0);
            Pose::delta(
                0.0,
                0.0,
                0.0,
                0.0,
                amount * amp * ((t * 26.0 + o).sin() + 0.57 * (t * 17.0 + o * 2.0).sin()),
            )
        }
        HabitKind::Bob => Pose::delta(
            0.0,
            -amount * h.dy.unwrap_or(0.0) * (t * speed * hz).sin().abs(),
            amount * h.rot_deg.unwrap_or(0.0) * (t * speed * hz * 0.65).sin(),
            0.0,
            0.0,
        ),
        HabitKind::Scuttle => {
            let m = (0.15 + (speed - 1.0).max(0.0) * 0.4).min(1.0);
            Pose::delta(
                0.0,
                -m * h.dy.unwrap_or(0.0) * (t * speed * hz).sin().abs(),
                m * h.rot_deg.unwrap_or(0.0) * (t * speed * hz).sin(),
                0.0,
                0.0,
            )
        }
        HabitKind::Wave => Pose::delta(
            0.0,
            0.0,
            amount * h.amp_deg.unwrap_or(0.0) * wave(1.0),
            amount * h.stretch.unwrap_or(0.0) * (t * speed * hz * 5.0 / 3.0).sin(),
            0.0,
        ),
    }
}

/// A burst of three taps at the start of each `every` seconds, then rest. 0 to 1.
fn taps(t: f32, speed: f32, every: f32) -> f32 {
    let ph = (t * speed) % every;
    if ph < 0.9 {
        (ph / 0.9 * PI * 3.0).sin().abs()
    } else {
        0.0
    }
}
