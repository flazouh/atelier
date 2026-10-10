use crate::enums::Paint;

/// The outline of a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeSpec {
    pub paint: Paint,
    pub width: f32,
    pub round_cap: bool,
    pub round_join: bool,
}
