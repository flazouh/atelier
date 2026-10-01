//! The review of what a session's agent changed: the review bar, the changed file tree, and the file's
//! card with the real editor and the inline review's hunks. One turn, or the whole session, by the
//! bar's switch.
//!
//! Each file is a `atelier_review::Merged`, the source of truth: the editor holds its text, a decision
//! calls `decide`, the reader's typing calls `edited`. The file on disk is `Merged::current()`: the pane
//! writes it through the Project at once after a decision, and a moment after the reader stops typing.
//! When the agent writes a file under review again (a later turn), the watch reports it; the pane reads
//! it, rebases the file's hunks on it, and puts the difference in the editor as one small edit, so the
//! caret and the scroll stay where they were.
//!
//! What the reader decided and marked is kept with the session, per scope and file, so the review opens
//! again as it was left. Comments go to the session, which sends them with the next message.

mod helpers;
mod structs;
mod types;

pub use structs::ReviewPane;
pub use types::{PaneEvent, Scope, SessionFor};

#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use atelier_ui::Decision;
#[cfg(test)]
use gpui_kit::{Context, Entity, IntoElement, ParentElement, Styled, Window, div};
#[cfg(test)]
use atelier_project::Project;
#[cfg(test)]
use crate::agent_session::AgentSession;

#[cfg(test)]
mod tests;
