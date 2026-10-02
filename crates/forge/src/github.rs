//! GitHub as a [`Forge`](crate::Forge): GraphQL where it can, REST where GraphQL lacks the thing (a job's steps and
//! log, review requests, deleting a branch). Requests go through `gh api` run by the project, so a
//! remote project uses its host's `gh` and its sign-in. `docs/forge.md` has the mapping and the limits.

mod briefs;
mod client;
mod gh_cli;
mod helpers;
mod involved;
mod queries;
mod read;
mod structs;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod transport;
mod wire;
mod write;

pub use gh_cli::GhCli;
pub use transport::{Reply, Request, Transport, TransportError};

pub use structs::GitHub;

use helpers::decode;

#[cfg(test)]
mod tests;
