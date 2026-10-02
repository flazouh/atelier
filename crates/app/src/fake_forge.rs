//! A forge for tests: it keeps what it was asked to write and answers from a script, and a repository
//! on disk with a pushed branch to open a pull request from. Nothing here reaches a network.

mod helpers;
mod structs;
mod types;

pub use helpers::{bare_of, git, pushed_branch};
pub use structs::FakeForge;
pub use types::SCRATCH_URL;
