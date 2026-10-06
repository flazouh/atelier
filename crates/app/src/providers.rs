//! Which provider a session runs on: one of the agent's accounts, or OpenRouter with the key the keychain holds.
//! A choice is kept in the settings as a key; the default for new sessions is one of them.
use std::sync::Arc;

use atelier_agents::session::{Account, ApiKey, Backend, Provider};
use atelier_project::{LocalProject, Project};
use atelier_settings::secrets::{InMemory, Keychain, OPENROUTER_KEY, Secrets};
use gpui_kit::{App, Global};

/// The words a session shows when it is set to OpenRouter and no key is kept.
pub const NO_KEY: &str = "No OpenRouter key yet. Add one in Settings, Providers.";

const ACCOUNT_PREFIX: &str = "account:";
const OPENROUTER: &str = "openrouter";
const USUAL_ACCOUNT: &str = atelier_agents::claude_code::accounts::DEFAULT_ACCOUNT;
/// What the usual account is called before its plan is known.
const USUAL_WORDS: &str = "Claude";
const OPENROUTER_WORDS: &str = "OpenRouter";
const KEY_CHECK_URL: &str = "https://openrouter.ai/api/v1/key";

/// A provider as the reader picks it. Unlike [`Provider`], it holds no key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Account(String),
    OpenRouter,
}

impl Choice {
    pub fn usual() -> Self {
        Self::Account(USUAL_ACCOUNT.into())
    }

    /// As the settings keep it.
    pub fn key(&self) -> String {
        match self {
            Self::Account(name) => format!("{ACCOUNT_PREFIX}{name}"),
            Self::OpenRouter => OPENROUTER.into(),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            OPENROUTER => Some(Self::OpenRouter),
            _ => key.strip_prefix(ACCOUNT_PREFIX).filter(|name| !name.is_empty()).map(|name| Self::Account(name.into())),
        }
    }

    /// The default the settings keep, or the usual account.
    pub fn saved(key: Option<&str>) -> Self {
        key.and_then(Self::from_key).unwrap_or_else(Self::usual)
    }
}

/// What the agent is started with for `choice`, the key read from `secrets`. Reads the keychain: keep it off the
/// UI thread.
pub fn provider(choice: &Choice, secrets: &dyn Secrets) -> Result<Provider, String> {
    match choice {
        Choice::Account(name) => Ok(Provider::Account(name.clone())),
        Choice::OpenRouter => match secrets.read(OPENROUTER_KEY) {
            Ok(Some(key)) => Ok(Provider::OpenRouter { key: ApiKey::new(key) }),
            Ok(None) => Err(NO_KEY.into()),
            Err(error) => Err(format!("The keychain could not be read: {error}")),
        },
    }
}

/// A choice's name in a menu: an account by its plan, then its own name unless it is the usual one.
pub fn label(choice: &Choice, accounts: &[Account]) -> String {
    let Choice::Account(name) = choice else { return OPENROUTER_WORDS.into() };
    let plan = accounts.iter().find(|a| &a.name == name).and_then(|a| a.plan.as_deref()).map(capitalised);
    match (plan, name == USUAL_ACCOUNT) {
        (Some(plan), true) => plan,
        (Some(plan), false) => format!("{plan} · {name}"),
        (None, true) => USUAL_WORDS.into(),
        (None, false) => name.clone(),
    }
}

/// The account a session signs in to: the one it runs on, else the usual one.
pub fn sign_in_account(provider: Option<&Choice>) -> String {
    match provider {
        Some(Choice::Account(name)) => name.clone(),
        _ => USUAL_ACCOUNT.into(),
    }
}

/// The account a notice names: only one the reader made, as the usual one needs no name.
pub fn named_account(provider: Option<&Choice>) -> Option<String> {
    Some(sign_in_account(provider)).filter(|name| name != USUAL_ACCOUNT)
}

/// The lab behind a choice: Anthropic for an account, OpenRouter for OpenRouter.
pub fn mark(choice: &Choice) -> Option<atelier_ui::BrandMark> {
    match choice {
        Choice::Account(_) => atelier_agents::labs::Lab::Anthropic.mark(),
        Choice::OpenRouter => atelier_agents::labs::Lab::OpenRouter.mark(),
    }
}

fn capitalised(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
}

/// Asks OpenRouter whether `key` works. Blocks on the network: keep it off the UI thread.
pub fn check_key(key: &str) -> Result<(), String> {
    match ureq::get(KEY_CHECK_URL).header("Authorization", &format!("Bearer {key}")).call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(401 | 403)) => Err("OpenRouter does not know this key".into()),
        Err(error) => Err(format!("OpenRouter could not be reached: {error}")),
    }
}

/// The provider new sessions start on, as the settings say.
pub struct DefaultProvider(pub Choice);

impl Global for DefaultProvider {}

pub fn default_choice(cx: &App) -> Choice {
    cx.try_global::<DefaultProvider>().map_or_else(Choice::usual, |d| d.0.clone())
}

/// The parts that reach past the app, which a test replaces: the keychain, OpenRouter's key check, and the agent's
/// accounts on this machine.
#[derive(Clone)]
pub struct ProviderServices {
    pub secrets: Arc<dyn Secrets>,
    pub check_key: fn(&str) -> Result<(), String>,
    pub accounts: fn() -> Result<Vec<Account>, String>,
}

impl Global for ProviderServices {}

impl ProviderServices {
    /// The system keychain, OpenRouter itself, and `claude auth status` on this machine.
    pub fn system() -> Self {
        Self { secrets: Arc::new(Keychain), check_key, accounts: accounts_here }
    }

    /// Memory for the keychain, and nothing reached: what a test gets unless it sets its own.
    pub fn isolated() -> Self {
        Self { secrets: Arc::new(InMemory::default()), check_key: |_| Err(NO_NETWORK.into()), accounts: || Ok(Vec::new()) }
    }
}

const NO_NETWORK: &str = "OpenRouter is not asked in a test";

pub fn services(cx: &App) -> ProviderServices {
    cx.try_global::<ProviderServices>().cloned().unwrap_or_else(ProviderServices::isolated)
}

pub fn secrets(cx: &App) -> Arc<dyn Secrets> {
    services(cx).secrets
}

/// The agent that runs on providers, and this machine as its host.
pub fn agent_here() -> Option<(Arc<dyn Backend>, Arc<dyn Project>)> {
    let agent = atelier_agents::registry::agents().into_iter().find(|a| a.backend.capabilities().providers)?;
    let home = std::env::var_os("HOME")?;
    let host: Arc<dyn Project> = Arc::new(LocalProject::open(home).ok()?);
    Some((agent.backend, host))
}

fn accounts_here() -> Result<Vec<Account>, String> {
    match agent_here() {
        Some((backend, host)) => backend.accounts(host.as_ref()).map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
