use serde::{Deserialize, Serialize};

/// The seven brand colours a bot may have.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    Pink,
    Purple,
    Blue,
    Green,
    Yellow,
    Orange,
    Red,
}
