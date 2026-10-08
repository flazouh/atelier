//! The `PATH` of an app that was not opened from a terminal. The Finder, the Dock and the updater that restarts the
//! app after an update start it with `/usr/bin:/bin:/usr/sbin:/sbin`, which finds no `claude`, no `git` from Homebrew
//! and no language server. The reader's login shell knows the `PATH` they mean, so the app asks it once, before any
//! thread starts, and keeps what it answers first. The folders installers use are added when the shell did not name
//! them. A shell that answers late, or not at all, leaves the `PATH` as it was.
mod helpers;

pub use helpers::adopt;

#[cfg(test)]
mod tests;
