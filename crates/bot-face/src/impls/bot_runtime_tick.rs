use std::f32::consts::PI;

use super::bot_runtime_new::fract;
use super::habit_pose::habit_pose;
use super::mood_pose::mood_pose;
use crate::consts::*;
use crate::enums::Mood;
use crate::structs::{BotModel, BotRuntime, FaceSet, Frame, Pose};

impl BotRuntime {
    /// Moves the bot to time `t` (seconds) and returns what to draw. `wanted` is the mood to show. `pointer` is
    /// the pointer's place relative to the bot, each way from -1 to 1, or none.
    pub fn tick(
        &mut self,
        set: &FaceSet,
        bot: &BotModel,
        t: f32,
        wanted: Mood,
        pointer: Option<(f32, f32)>,
    ) -> Frame {
        let dt = self
            .last_t
            .map_or(1.0 / REFERENCE_FPS, |l| (t - l).clamp(0.0, 0.1));
        self.last_t = Some(t);
        let reaction = self.reaction_progress(t);
        let mood = if reaction.is_some() {
            Mood::Done
        } else {
            wanted
        };
        let (pose, speed, amount) = self.blend(set, t, mood, dt);
        let mut root = pose;
        if let Some(u) = reaction {
            react_pose(&mut root, u);
        }
        let grow = ease_out_back((t - self.delay) / APPEAR_SECONDS);
        root.sx *= grow;
        root.sy *= grow;
        root.y += (1.0 - grow) * 18.0;
        let (dx, dy) = pointer.unwrap_or((0.0, 0.0));
        let look = 1.0 - self.weights[Mood::Stuck.index()];
        root.r += dx * LEAN_DEGREES * look;
        let parts = bot
            .parts
            .iter()
            .map(|p| {
                p.habit.as_ref().map_or_else(Pose::default, |h| {
                    habit_pose(h, p.pivot, t, speed, amount, self.phase)
                })
            })
            .collect();
        let w = self.weights;
        Frame {
            root,
            parts,
            eye_weights: w,
            blink: self.blink(t),
            scan_x: 8.0 * (t * 5.0).sin(),
            look: (dx * LOOK_X * look, dy * LOOK_Y * look),
            cheeks: w[Mood::Idle.index()]
                .max(w[Mood::Done.index()])
                .max(w[Mood::Thinking.index()]),
        }
    }

    /// 0 to 1 while a click reaction runs, else none.
    fn reaction_progress(&mut self, t: f32) -> Option<f32> {
        let u = (t - self.react_at?) / REACT_SECONDS;
        if (0.0..1.0).contains(&u) {
            Some(u)
        } else {
            if u >= 1.0 {
                self.react_at = None;
            }
            None
        }
    }

    /// Moves each mood's weight toward its target, then adds the poses, the speed and the amount by weight.
    fn blend(&mut self, set: &FaceSet, t: f32, mood: Mood, dt: f32) -> (Pose, f32, f32) {
        let k = 1.0 - (1.0 - BLEND_PER_FRAME).powf(dt * REFERENCE_FPS);
        let (mut x, mut y, mut r, mut sx, mut sy, mut speed, mut amount) =
            (0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0);
        for m in Mood::ALL {
            let i = m.index();
            self.weights[i] += ((if m == mood { 1.0 } else { 0.0 }) - self.weights[i]) * k;
            let w = self.weights[i];
            if w < 0.003 {
                continue;
            }
            let p = mood_pose(m, t, self.phase);
            x += w * p.x;
            y += w * p.y;
            r += w * p.r;
            sx += w * (p.sx - 1.0);
            sy += w * (p.sy - 1.0);
            let s = set.state(m);
            speed += w * s.speed;
            amount += w * s.amount;
        }
        (Pose { x, y, r, sx, sy }, speed, amount)
    }

    /// The eye height while the bot blinks: 1 open, down to 0.08 shut.
    fn blink(&mut self, t: f32) -> f32 {
        if self.blink_start.is_none() && t >= self.next_blink {
            self.blink_start = Some(t);
        }
        let Some(start) = self.blink_start else {
            return 1.0;
        };
        let u = (t - start) / BLINK_SECONDS;
        if u >= 1.0 {
            self.blink_start = None;
            self.next_blink = t
                + BLINK_GAP_MIN
                + fract((t * 12.9898 + self.phase * 78.233).sin() * 43758.547) * BLINK_GAP_SPREAD;
            1.0
        } else {
            1.0 - 0.92 * (PI * u).sin()
        }
    }
}

/// One click: crouch, jump and land with a squash. `u` runs from 0 to 1.
fn react_pose(p: &mut Pose, u: f32) {
    if u < 0.2 {
        let k = u / 0.2;
        p.sy -= 0.2 * k;
        p.sx += 0.12 * k;
    } else if u < 0.85 {
        let v = (u - 0.2) / 0.65;
        p.y -= 18.0 * 4.0 * v * (1.0 - v);
        p.sy += 0.1 * (PI * v).sin();
        p.sx -= 0.05 * (PI * v).sin();
    } else {
        let k = (1.0 - u) / 0.15;
        p.sy -= 0.15 * k;
        p.sx += 0.1 * k;
    }
}

/// A growth with a small overshoot. 0 before the start, 1 after the end.
fn ease_out_back(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let (c1, c3) = (1.9, 2.9);
    1.0 + c3 * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}
