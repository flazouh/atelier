//! The pull request the gallery story shows: a small Rust crate, "relay", on main; and pull request 3344,
//! which detaches a byte stream when a client aborts. The repository is real git, made on disk. The forge is
//! the in-memory one, holding the pull request's threads, remarks, checks and a failing job with its log.

mod helpers;
mod structs;
mod types;

pub use helpers::involved;
pub use structs::Relay;
pub use types::{BODY, NUMBER};
