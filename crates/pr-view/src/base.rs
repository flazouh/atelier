//! Which commit the diff starts from. The whole pull request starts at the merge base. **Since Last
//! Review** starts at the commit the reader last reviewed up to, so a second look shows only what was pushed
//! after the first. Any commit of the pull request can be the base too. A commit that is not part of the
//! branch any more (a force push rewrote it) cannot be a base, and the diff falls back to the whole pull
//! request and says why.

mod helpers;
mod structs;
mod types;

pub use helpers::{opening_choice, resolve};
pub use structs::Base;
pub use types::BaseChoice;
