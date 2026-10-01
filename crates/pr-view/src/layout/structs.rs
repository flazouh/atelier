use super::types::Part;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The rail's width, or `None` when it is folded.
    pub rail: Option<f32>,
    pub tree: bool,
    /// In a pane under [`NARROW`], the one part that shows.
    pub single: Option<Part>,
}
