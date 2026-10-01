//! What the reader has seen, kept on this machine. **Reviewed State** is per file version: a file is seen
//! for the version (the blob) it had when it was marked, so a push that changes the file makes it unseen
//! again, and a push that leaves it alone does not. It is a small SQLite database in the app's data
//! folder, the tracker's pattern: migrations by `user_version`, WAL.

mod helpers;
mod structs;
mod types;

pub use helpers::is_seen;
pub use structs::Reviewed;
