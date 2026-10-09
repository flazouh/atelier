//! Where the Tasks screen gets its tasks: the providers a project has (`atelier_capabilities::Registry`), the one
//! the reader chose, and what that one offers right now. The screen reads and changes tasks through the chosen
//! `TasksProvider` and never through the local tracker; the tracker stays only for the rules and the session and
//! pull request links, which are local-only until they move to the capability.
mod structs;

pub use structs::{Choice, TasksSource, Vocabulary};

#[cfg(test)]
mod tests;
