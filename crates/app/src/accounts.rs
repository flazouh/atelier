//! The accounts a person connected: Linear with an API key, GitHub Issues of one repository over the `gh` login, and
//! Slack, Discord and Gmail through the reader's own `slackcli`, `discordcli` and `gmailcli` (see [`cli`]), whose logins
//! stay with those tools. [`build`] turns the saved facts into providers, which the
//! [`CapabilityHub`](crate::capability_hub::CapabilityHub) holds for the Tasks, Messages and Mail screens and the agent
//! gateway alike; [`refresh`] does that off the UI thread and tells the screens that read it.
//! A provider that cannot be built (a refused key, no network, a tool that is not installed) is left out and shows as a
//! state of its row in Settings. One that fails never hides another.
mod cli;
mod helpers;
mod structs;
mod types;

#[cfg(test)]
pub(crate) use helpers::build;
pub(crate) use helpers::{
    connect_discord, connect_github, connect_gmail, connect_linear, connect_slack, plain_words,
    refresh,
};

pub(crate) use cli::split_list;
pub(crate) use structs::{AccountServices, Built, Rows, services};
pub(crate) use types::{Kind, Row};

#[cfg(test)]
mod tests;
