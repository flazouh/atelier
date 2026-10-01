//! A real git repository on disk, for tests, the story and the numbers: a bare "forge" repository, a project cloned from it, and pull requests
//! made in a scratch clone and pushed as `refs/pull/N/head`, the way GitHub keeps them. The project never
//! has the pull request's objects, so `prepare` must fetch them.

mod helpers;
mod structs;

pub use helpers::{git, git_ok, put, remove};
pub use structs::Repo;
