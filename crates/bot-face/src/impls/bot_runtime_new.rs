use crate::enums::Mood;
use crate::structs::BotRuntime;

impl BotRuntime {
    /// A runtime for one bot. `seed` sets its phase and its first blink, so bots do not move together. `delay`
    /// is the time, in seconds, before the bot grows in.
    pub fn new(seed: u32, delay: f32) -> BotRuntime {
        let phase = (seed as f32 * 2.399_963).rem_euclid(std::f32::consts::TAU);
        let mut weights = [0.0; 6];
        weights[Mood::Idle.index()] = 1.0;
        BotRuntime {
            weights,
            phase,
            delay,
            last_t: None,
            next_blink: 1.0 + fract((phase * 12.9898).sin() * 43758.547) * 3.0,
            blink_start: None,
            react_at: None,
        }
    }

    /// Starts one click reaction at time `t`.
    pub fn react(&mut self, t: f32) {
        self.react_at = Some(t);
    }
}

/// The part of a number after the point, 0 to 1.
pub(super) fn fract(x: f32) -> f32 {
    x - x.floor()
}
