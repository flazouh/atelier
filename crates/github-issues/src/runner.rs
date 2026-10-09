//! How the provider reaches GitHub: one call to `gh api`, behind the [`Gh`] trait so a test can replace it.
mod helpers;
mod structs;
mod traits;
mod types;

pub use structs::{Call, GhCli, Reply};
pub use traits::Gh;
pub use types::{Failure, Method};

#[cfg(test)]
mod tests;
