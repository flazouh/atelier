//! A pull request kept on disk, so the next open draws from it at once and the forge only brings what
//! changed. One JSON file per pull request in the app's data folder, written whole through a temporary
//! name. A file that does not read (a newer or older atelier wrote it, or the disk lost a byte) is a miss,
//! never an error.

mod structs;
mod types;

pub use structs::{ListSnapshot, Snapshots};
