//! `#N` in agent text, looked up without waiting on the forge every time: many numbers in one request,
//! answers kept for a short while, the current repository first and then the pull requests the reader
//! is involved in.

mod structs;
mod types;

pub use structs::Lookup;

#[cfg(test)]
mod tests;
