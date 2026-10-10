use serde::{Deserialize, Serialize};

/// How a bot's body moves. The names match the shapes in `faces.v1.json`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Body {
    Tank,
    Wheels,
    Spring,
    Hover,
    Legs,
    Feet,
    Ball,
}
