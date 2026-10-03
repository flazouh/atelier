//! The window's projects and sessions as the sidebar and the agent panels take them: plain data, made
//! fresh from the open projects whenever a session or a project changes.
//!
//! A session open in the window is keyed by its panel's key. A past one, which the agent lists but no
//! panel holds, is keyed `past:<the agent's id>`, so opening it resumes it.

mod helpers;
mod structs;
mod types;

pub use helpers::{newest_first, panels, pick, project_id, sidebar};
#[cfg(test)]
pub use helpers::badge_of;
pub use structs::Badges;
pub use types::Pick;

#[cfg(test)]
use atelier_ui::project_badge;

#[cfg(test)]
mod tests;
