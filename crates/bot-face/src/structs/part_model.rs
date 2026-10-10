use super::{HabitDef, Shape};
use crate::enums::Layer;

/// A part with its shapes parsed.
#[derive(Clone, Debug, PartialEq)]
pub struct PartModel {
    pub layer: Layer,
    pub shapes: Vec<Shape>,
    pub pivot: (f32, f32),
    pub habit: Option<HabitDef>,
}
