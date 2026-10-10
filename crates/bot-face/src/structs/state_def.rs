use serde::Deserialize;

/// How a mood scales every habit.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct StateDef {
    pub speed: f32,
    pub amount: f32,
}
