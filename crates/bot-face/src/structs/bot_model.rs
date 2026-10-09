use gpui_kit::Rgba;

use super::{PartModel, Shape};

/// A bot ready to move and draw. Parts keep the order of the data.
#[derive(Clone, Debug, PartialEq)]
pub struct BotModel {
    pub id: String,
    pub name: String,
    pub role: String,
    pub colour: Rgba,
    pub eye_y: f32,
    pub parts: Vec<PartModel>,
    /// One set of shapes for each mood, in the order of `Mood::ALL`.
    pub eyes: Vec<Vec<Shape>>,
    pub cheeks: Vec<Shape>,
}
