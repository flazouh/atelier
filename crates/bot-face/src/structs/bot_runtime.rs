/// The moving state of one bot on screen. Make one for each bot, and call `tick` each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct BotRuntime {
    pub(in super::super) weights: [f32; 6],
    pub(in super::super) phase: f32,
    pub(in super::super) delay: f32,
    pub(in super::super) last_t: Option<f32>,
    pub(in super::super) next_blink: f32,
    pub(in super::super) blink_start: Option<f32>,
    pub(in super::super) react_at: Option<f32>,
}
