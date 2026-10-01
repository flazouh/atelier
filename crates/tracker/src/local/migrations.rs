//! The schema, one step at a time. `PRAGMA user_version` counts the steps applied. A step is never edited
//! once released: a change is a new step at the end of [`STEPS`].

mod helpers;
mod types;

pub use helpers::run;
#[cfg(test)]
pub use types::CURRENT;
