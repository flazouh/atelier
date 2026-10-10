use serde::{Deserialize, Serialize};

use crate::enums::{Body, Colour, Tool};

/// What a bot looks like: a body, a colour and a tool. `atelier-bot-face` draws it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct FaceChoice {
    pub body: Body,
    pub colour: Colour,
    pub tool: Tool,
}
