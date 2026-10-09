//! The accounts a person connected for their tasks: Linear with an API key, GitHub Issues of one repository over the
//! `gh` login. [`build`] turns the saved facts into providers, which the [`CapabilityHub`](crate::capability_hub::CapabilityHub)
//! holds for the Tasks screen and the agent gateway alike; [`refresh`] does that off the UI thread and tells the screens.
//! A provider that cannot be built (a refused key, no network) is left out and shows as a state of its row in Settings.
mod helpers;
mod structs;
mod types;

#[cfg(test)]
pub(crate) use helpers::build;
pub(crate) use helpers::{connect_github, connect_linear, plain_words, refresh};
pub(crate) use structs::{AccountServices, Built, Rows, services};
pub(crate) use types::{Kind, Row};

#[cfg(test)]
mod tests;
