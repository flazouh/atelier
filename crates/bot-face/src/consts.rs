//! The numbers the player shares.

/// The side of the square every bot is drawn in, before it is scaled to the screen.
pub const VIEW: f32 = 120.0;
/// The line where the bots stand, in view units. Squash and tilt pivot here.
pub const GROUND: f32 = 104.0;
/// A mood's weight moves this share of the way to its target each frame at 60 frames a second.
pub const BLEND_PER_FRAME: f32 = 0.12;
/// The frame rate the blend share is written for.
pub const REFERENCE_FPS: f32 = 60.0;
/// A click reaction lasts this many seconds.
pub const REACT_SECONDS: f32 = 0.75;
/// A bot takes this many seconds to grow in.
pub const APPEAR_SECONDS: f32 = 0.8;
/// A blink lasts this many seconds.
pub const BLINK_SECONDS: f32 = 0.16;
/// The shortest gap between two blinks, in seconds.
pub const BLINK_GAP_MIN: f32 = 2.0;
/// The random part of the gap between two blinks, in seconds.
pub const BLINK_GAP_SPREAD: f32 = 3.5;
/// The farthest the eyes move sideways toward the pointer, in view units.
pub const LOOK_X: f32 = 3.2;
/// The farthest the eyes move up or down toward the pointer, in view units.
pub const LOOK_Y: f32 = 2.2;
/// The farthest the body leans toward the pointer, in degrees.
pub const LEAN_DEGREES: f32 = 1.6;
/// The control point distance that makes a cubic curve a quarter circle.
pub const KAPPA: f32 = 0.552_284_7;
