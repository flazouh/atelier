//! What the gallery's measuring stories share: how long a frame took against the display's interval, the
//! medians after a run of frames, and the layout node count of a tree. `GALLERY_SCROLL=1` runs a story's
//! scroll and prints them. `docs/performance.md` says how to read the numbers.

mod helpers;
mod structs;
mod types;

pub use helpers::{report, report_count};
pub use structs::{Stages, Timed};
pub use types::FRAMES;
