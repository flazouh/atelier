//! One agent session in a project: the agent's own session, the queue its events land in, the
//! `Conversation` they fold into, and the list that draws it.
//!
//! Events arrive on the agent's threads. The queue joins a block's deltas and wakes this entity once
//! when it goes from empty to not; the entity then drains everything waiting, folds it, and asks for
//! one repaint, so a fast stream costs a repaint a frame. Only the rows whose content changed are
//! measured again (`list_diff`), so the list keeps its scroll while text streams.
//!
//! Each turn is tracked for review (`atelier-review`): the turn's `begin` (a git snapshot) runs on a
//! background task before the message goes to the agent; every event then passes the tracker on the
//! agent's own thread, as it arrives, so a file is read before the tool that names it writes it; and the
//! turn's end `finish`es it there too. The panel shows the turn's changed files after its last row.

mod composer_lists;
pub(crate) mod dictation;
pub mod handoff;
mod helpers;
mod queue;
mod structs;
mod types;

pub use helpers::now;
pub use structs::AgentSession;
pub(crate) use structs::{RunPickedSkills, runs_picked_skills};
pub use types::SessionEvent;

#[cfg(test)]
use types::SAVE_AFTER;

#[cfg(test)]
use atelier_ui::session_status::SessionStatus;
#[cfg(test)]
use gpui_kit::Entity;
#[cfg(test)]
use atelier_agents::session::{ChoiceKind, Command, Event};
#[cfg(test)]
use crate::list_diff;

#[cfg(test)]
mod tests;
