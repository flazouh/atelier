//! Serves every agent's strips to GPUI, then everything atelier-ui serves.

mod helpers;
mod structs;

#[cfg(test)]
pub(crate) use helpers::strip_bytes;
pub use structs::Assets;

#[cfg(test)]
use crate::claude;

#[cfg(test)]
mod tests;
