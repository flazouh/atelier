//! A check's log, read for the one line that says why it failed.

mod helpers;

pub use helpers::first_error_line;

#[cfg(test)]
mod tests;
