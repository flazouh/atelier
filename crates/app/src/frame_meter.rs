//! `ATELIER_FRAMES=1`: how long each frame's layout and paint take on the CPU, over the whole window,
//! reported every 300 frames as a median, a p95, a worst and a count over 8 ms (120 Hz). The window's
//! root is wrapped in [`Timed`]; paint here is building the scene, which the GPU draws later.
//! `ATELIER_FRAMES=each` also prints one line per frame, `frame <unix ms> <cpu ms>`, to count the frames of an
//! idle screen or to take one animation's median and p95.

mod helpers;
mod structs;
mod types;

pub use helpers::{add_part, enabled, last_frame};
#[cfg(test)]
pub use helpers::{each_line, mode};
pub use structs::{Meter, Part, Timed};
#[cfg(test)]
pub use types::Mode;

#[cfg(test)]
mod tests;
