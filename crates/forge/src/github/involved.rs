//! The reader's working set: ten searches in flight at once.

mod helpers;
mod types;

pub(super) use helpers::involved;

#[cfg(test)]
use helpers::searches;
#[cfg(test)]
use types::SEARCHES;

#[cfg(test)]
mod tests;
