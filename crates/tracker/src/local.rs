//! The local tracker: one SQLite database per project, in the app's data folder. Every call blocks, so the
//! app makes none on the UI thread. One connection behind a lock; the database is in WAL mode, so a write
//! is one small append and does not wait for the disk to settle.

mod helpers;
mod impls;
mod migrations;
mod structs;
mod types;

pub use structs::LocalTracker;

#[cfg(test)]
mod tests;
