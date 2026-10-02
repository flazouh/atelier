//! What every view of this crate shares: the forge, git, the caches on disk and the reader's settings.

mod helpers;
mod structs;

pub use helpers::now;
pub use structs::{PrConfig, Services};
