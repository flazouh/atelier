//! The bot faces. `docs/bots/faces.v1.json` holds seven robots as plain shapes, with a pose for each mood and a
//! habit for some parts. This crate reads that data and draws the robots with GPUI paths. `FaceSet` holds the
//! data, `BotRuntime` moves one bot from frame to frame, and `paint_bot` draws a `Frame`.

mod consts;
mod enums;
mod impls;
mod structs;
#[cfg(test)]
mod tests;

pub use enums::{Geometry, HabitKind, Layer, Mood, Paint, PathCmd, Token};
pub use impls::paint_bot;
pub use structs::{
    Affine, BotDef, BotModel, BotRuntime, FaceData, FaceSet, Frame, HabitDef, Palette, PartDef,
    PartModel, Pose, Shape, StateDef, StrokeSpec,
};
