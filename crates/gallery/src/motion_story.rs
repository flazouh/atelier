//! The "Motion" story: the components ported from beui.dev's `motion/` and `blocks/`, each in every state it
//! has. `MOTION_PART=<name>` shows one alone, at the top left of the page, so a screenshot of it can be laid
//! beside the web demo's (`~/shots/beui/<name>-compare.png`). Without it, every part is listed.

mod helpers;
mod structs;
mod types;

pub use structs::MotionStory;
