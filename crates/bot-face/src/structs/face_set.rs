use gpui_kit::Rgba;

use super::{BotModel, StateDef};

/// All the bots of one data file, with the numbers the moods share.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceSet {
    pub bots: Vec<BotModel>,
    /// The speed and the amount of each mood, in the order of `Mood::ALL`.
    pub(in super::super) states: [StateDef; 6],
    pub(in super::super) grey: Rgba,
    pub(in super::super) mix: f32,
}
