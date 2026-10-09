use std::sync::Arc;

use atelier_capabilities::{
    CapError, CapResult, mail::MailProvider, messaging::MessagingProvider, tasks::TasksProvider,
};
use atelier_settings::{
    DiscordSaved, GmailSaved, SlackSaved,
    secrets::{InMemory, Keychain, Secrets},
};
use gpui_kit::{App, Global};

use super::{cli, helpers, types::Row};

/// The state of each kind's row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rows {
    pub linear: Row,
    pub github: Row,
    pub slack: Row,
    pub discord: Row,
    pub gmail: Row,
}

/// What [`build`](super::build) made: the providers that work, and the state of each row.
#[derive(Default)]
pub(crate) struct Built {
    pub rows: Rows,
    pub providers: Vec<Arc<dyn TasksProvider>>,
    pub messaging: Vec<Arc<dyn MessagingProvider>>,
    pub mail: Vec<Arc<dyn MailProvider>>,
}

/// The parts that reach past the app, which a test replaces: the keychain, and the ways to connect. Each way builds
/// a provider from what is saved; the caller then asks the service who it signed in as. Both block on the network or on a
/// command line tool, so they never run on the UI thread.
#[derive(Clone)]
pub(crate) struct AccountServices {
    pub secrets: Arc<dyn Secrets>,
    /// From an API key.
    pub linear: fn(&str) -> CapResult<Arc<dyn TasksProvider>>,
    /// From `owner/repo`, over the `gh` login.
    pub github: fn(&str) -> CapResult<Arc<dyn TasksProvider>>,
    /// Over the reader's `slackcli`.
    pub slack: fn(&SlackSaved) -> CapResult<Arc<dyn MessagingProvider>>,
    /// Over the reader's `discordcli`.
    pub discord: fn(&DiscordSaved) -> CapResult<Arc<dyn MessagingProvider>>,
    /// Over the reader's `gmailcli`, here or over SSH.
    pub gmail: fn(&GmailSaved) -> CapResult<Arc<dyn MailProvider>>,
}

impl Global for AccountServices {}

impl AccountServices {
    /// The system keychain, Linear itself, the `gh` on the path, and the three command line tools.
    pub(crate) fn system() -> Self {
        Self {
            secrets: Arc::new(Keychain),
            linear: helpers::linear_system,
            github: helpers::github_system,
            slack: cli::slack_system,
            discord: cli::discord_system,
            gmail: cli::gmail_system,
        }
    }

    /// Memory for the keychain and no network: what a test gets unless it sets its own.
    pub(crate) fn isolated() -> Self {
        Self {
            secrets: Arc::new(InMemory::default()),
            linear: |_| Err(CapError::Offline),
            github: |_| Err(CapError::Offline),
            slack: |_| Err(CapError::Offline),
            discord: |_| Err(CapError::Offline),
            gmail: |_| Err(CapError::Offline),
        }
    }
}

/// The services the app has, else the isolated ones.
pub(crate) fn services(cx: &App) -> AccountServices {
    cx.try_global::<AccountServices>()
        .cloned()
        .unwrap_or_else(AccountServices::isolated)
}
