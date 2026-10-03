//! The Settings page: what a reader can set, in four sections with a list of them at its left. Appearance holds the
//! theme, light, dark or the system's, and the primary colour: the fill of the main button, and the accent and the
//! selection wash too (see `atelier_ui::theme::with_pick`). Agents lists the agents this build can start and the models
//! each offers. Providers holds the agent's accounts, the OpenRouter key kept in the keychain, and the provider new
//! sessions start on. Tasks holds the rules that move a task by itself. Keys lists the review's key table, read only for
//! now. A change applies at once, to every window, and is kept in `atelier-settings`. Escape closes the page, and so
//! does the Back button in the title bar.

mod helpers;
mod providers;
mod structs;
mod types;

pub use helpers::colour;
#[cfg(test)]
pub(crate) use helpers::rule_switch;
pub use structs::{AgentRow, SettingsPane};
pub use types::{Mode, SettingsEvent};
#[cfg(test)]
pub use types::{PRIMARIES, Section};

#[cfg(test)]
use atelier_ui::scale::px;

#[cfg(test)]
mod tests;
