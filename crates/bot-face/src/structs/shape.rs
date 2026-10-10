use super::StrokeSpec;
use crate::enums::{Geometry, Paint};

/// One drawn shape, parsed from the SVG text of a part.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub geometry: Geometry,
    pub fill: Option<Paint>,
    pub stroke: Option<StrokeSpec>,
    /// The share of the paint's own alpha that shows.
    pub opacity: f32,
    /// A turn in degrees around a point, written `transform="rotate(a cx cy)"`.
    pub spin: Option<[f32; 3]>,
}
