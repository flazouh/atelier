//! The Agent panel story: a whole session as the panel will show it, at the panel's width. A finished
//! turn ends with its changed files; a subagent runs where it started; the subagent strip and the
//! session's pull request sit above the composer; and the reply names #3344, which the app knows, and
//! #9999, which it does not.

mod helpers;
mod types;

pub use helpers::agent_panel;
pub use types::SESSION_LEN;
