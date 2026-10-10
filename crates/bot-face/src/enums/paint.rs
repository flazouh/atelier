use gpui_kit::Rgba;

use super::Token;

/// What a shape is filled or stroked with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Token(Token),
    Literal(Rgba),
}
