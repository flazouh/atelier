//! The Settings page: what a reader can set, in four sections with a list of them at its left. Appearance holds the
//! theme, light, dark or the system's, and the primary colour: the fill of the main button, and the accent and the
//! selection wash too (see `atelier_ui::theme::with_pick`). Agents lists the agents this build can start and the models
//! each offers. Providers holds the agent's accounts, the OpenRouter key kept in the keychain, and the provider new
//! sessions start on. Accounts connects Linear (an API key in the keychain) and GitHub Issues (one repository, over the
//! `gh` login) for the Tasks screen and the agent gateway. Tasks holds the rules that move a task by itself. Keys lists the review's key table, read only for
//! now. A change applies at once, to every window, and is kept in `atelier-settings`. Escape closes the page, and so
//! does the Back button in the title bar.

mod accounts;
mod helpers;
mod providers;
mod structs;
pub(crate) mod strings;
mod types;

pub use helpers::colour;
pub(crate) use helpers::save;
#[cfg(test)]
pub(crate) use helpers::rule_switch;
pub use structs::{AgentRow, SettingsPane};
pub use types::{Mode, SettingsEvent};
pub use types::Section;
#[cfg(test)]
pub use types::PRIMARIES;

#[cfg(test)]
use atelier_ui::scale::px;

#[cfg(test)]
mod tests;
