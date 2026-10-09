use std::sync::Arc;

use atelier_capabilities::{CapError, CapResult, tasks::TasksProvider};
use atelier_settings::secrets::{InMemory, Keychain, Secrets};
use gpui_kit::{App, Global};

use super::{helpers, types::Row};

/// The state of each kind's row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rows {
    pub linear: Row,
    pub github: Row,
}

/// What [`build`](super::build) made: the providers that work, and the state of each row.
pub(crate) struct Built {
    pub rows: Rows,
    pub providers: Vec<Arc<dyn TasksProvider>>,
}

/// The parts that reach past the app, which a test replaces: the keychain, and the two ways to connect. Each way builds
/// a provider and asks the service who it signed in as, so it blocks on the network and never runs on the UI thread.
#[derive(Clone)]
pub(crate) struct AccountServices {
    pub secrets: Arc<dyn Secrets>,
    /// From an API key.
    pub linear: fn(&str) -> CapResult<Arc<dyn TasksProvider>>,
    /// From `owner/repo`, over the `gh` login.
    pub github: fn(&str) -> CapResult<Arc<dyn TasksProvider>>,
}

impl Global for AccountServices {}

impl AccountServices {
    /// The system keychain, Linear itself, and the `gh` on the path.
    pub(crate) fn system() -> Self {
        Self { secrets: Arc::new(Keychain), linear: helpers::linear_system, github: helpers::github_system }
    }

    /// Memory for the keychain and no network: what a test gets unless it sets its own.
    pub(crate) fn isolated() -> Self {
        Self { secrets: Arc::new(InMemory::default()), linear: |_| Err(CapError::Offline), github: |_| Err(CapError::Offline) }
    }
}

/// The services the app has, else the isolated ones.
pub(crate) fn services(cx: &App) -> AccountServices {
    cx.try_global::<AccountServices>().cloned().unwrap_or_else(AccountServices::isolated)
}
