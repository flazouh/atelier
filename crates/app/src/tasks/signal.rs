//! What a session tells its tasks. The session says three things (it started, a turn ended, the reader
//! replied); this turns each into the tracker's `Signal`, which the rules read. Pure.

mod helpers;
mod structs;
mod types;

pub use helpers::of;
pub use structs::SessionRef;
pub use types::TaskEvent;
