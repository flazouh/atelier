//! `ATELIER_TIMINGS=1`: when the first frame showed, and when the first frame with the restored project and
//! session showed, both counted from the start of the process.

mod helpers;
mod types;

pub use helpers::{enabled, mark_start, since_start};
