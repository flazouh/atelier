//! The box for an account that reached its plan's usage limit: which limit, when it resets, and a way to
//! go on now with another agent or provider ("Handoff").

mod helpers;
mod types;

pub use helpers::limit_notice;

#[cfg(test)]
mod tests;
