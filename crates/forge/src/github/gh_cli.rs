//! The transport that runs `gh api` through the project, so a remote project uses its host's `gh`. `gh`
//! holds the token and adds it to the request itself: it never passes through atelier, so atelier has none
//! to store, log or print.

mod helpers;
mod structs;
mod types;

#[cfg(test)]
pub(super) use helpers::classify;
#[cfg(test)]
pub(super) use helpers::parse_reply;
pub use structs::GhCli;

#[cfg(test)]
mod tests;
