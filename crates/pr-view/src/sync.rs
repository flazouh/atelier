//! Keeping a pull request current while it is open. A refresh asks the forge for the header first; when the
//! header is the one the reader has, nothing else is asked, so a quiet pull request costs one small call.
//! When it changed, the parts that change while people work are read again, and the files too when the
//! head moved. How often to ask is [`Cadence`]: often while things happen, less while they do not, and
//! further apart after failures, with a rate limit obeyed. Nothing here draws or blocks the UI.

mod helpers;
mod structs;

pub use helpers::{delta, keep_place, refresh};
pub use structs::{Cadence, Delta, Refreshed};
