use serde::Deserialize;

use super::HabitDef;
use crate::enums::Layer;

/// One part of a bot as the data writes it: shapes as SVG text, a pivot and an optional habit.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PartDef {
    pub layer: Layer,
    pub svg: String,
    pub pivot: [f32; 2],
    #[serde(default)]
    pub habit: Option<HabitDef>,
}
