use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError, CapResult,
    mail::MailProvider,
    messaging::MessagingProvider,
    tasks::{Query, TasksProvider},
};
use atelier_settings::{
    AccountsSaved, DiscordSaved, GmailSaved, SlackSaved, secrets::LINEAR_KEY,
};
use gpui_kit::{App, AppContext, BorrowAppContext};

use super::{
    structs::{AccountServices, Built, Rows, services},
    types::{Kind, Row},
};
use crate::capability_hub::CapabilityHub;

pub(super) fn linear_system(key: &str) -> CapResult<Arc<dyn TasksProvider>> {
    Ok(Arc::new(atelier_linear::LinearTasks::connect(key)?))
}

pub(super) fn github_system(repo: &str) -> CapResult<Arc<dyn TasksProvider>> {
    Ok(Arc::new(atelier_github_issues::GithubIssues::through_gh(
        repo,
    )?))
}

/// The name a person goes by in the service.
fn name_of(actor: &Actor) -> String {
    actor.name.clone()
}

/// Signs in to Linear with `key` and asks who it is.
pub(crate) fn connect_linear(
    services: &AccountServices,
    key: &str,
) -> CapResult<(Arc<dyn TasksProvider>, String)> {
    let provider = (services.linear)(key)?;
    let name = name_of(&provider.whoami()?);
    Ok((provider, name))
}

/// Opens `repo` over the `gh` login: asks who it is, and reads one issue so a repository `gh` cannot see fails here.
pub(crate) fn connect_github(
    services: &AccountServices,
    repo: &str,
) -> CapResult<(Arc<dyn TasksProvider>, String)> {
    let provider = (services.github)(repo.trim())?;
    let name = name_of(&provider.whoami()?);
    provider.list(&Query {
        limit: Some(1),
        ..Query::default()
    })?;
    Ok((provider, name))
}

/// Asks `slackcli` who is signed in. The first call is the cheapest real one the provider has.
pub(crate) fn connect_slack(
    services: &AccountServices,
    saved: &SlackSaved,
) -> CapResult<(Arc<dyn MessagingProvider>, String)> {
    let provider = (services.slack)(saved)?;
    let name = name_of(&provider.whoami()?);
    Ok((provider, name))
}

/// Asks `discordcli` who is signed in.
pub(crate) fn connect_discord(
    services: &AccountServices,
    saved: &DiscordSaved,
) -> CapResult<(Arc<dyn MessagingProvider>, String)> {
    let provider = (services.discord)(saved)?;
    let name = name_of(&provider.whoami()?);
    Ok((provider, name))
}

/// Asks `gmailcli` which account the browser is signed in to, and checks it is the address the reader gave. The name of a
/// Gmail account is its address.
pub(crate) fn connect_gmail(
    services: &AccountServices,
    saved: &GmailSaved,
) -> CapResult<(Arc<dyn MailProvider>, String)> {
    let provider = (services.gmail)(saved)?;
    let address = provider.whoami()?.address;
    Ok((provider, address))
}

/// A failure in words a person can act on.
pub(crate) fn plain_words(kind: Kind, error: &CapError) -> String {
    match error {
        CapError::NotSignedIn => kind.refusal().into(),
        CapError::Offline => format!(
            "{} could not be reached. Check the connection.",
            kind.name()
        ),
        CapError::RateLimited { retry_after_ms } => format!(
            "{} asks to wait {} s before the next try.",
            kind.name(),
            retry_after_ms.div_ceil(1000)
        ),
        CapError::NotFound { .. } if kind == Kind::GithubIssues => {
            "GitHub does not show this repository to your gh login.".into()
        }
        CapError::NotFound { .. } if kind == Kind::Discord => {
            "Discord does not show a server with this name to your login.".into()
        }
        CapError::NotFound { .. } if kind == Kind::Slack => {
            "Slack does not show a channel with this name or id to your login.".into()
        }
        CapError::Invalid { .. } if kind == Kind::GithubIssues => {
            "Write the repository as owner/repo.".into()
        }
        CapError::Invalid { .. } if kind == Kind::Slack => {
            "Write the workspace's name. It names the account in references.".into()
        }
        CapError::Invalid { .. } if kind == Kind::Discord => {
            "Name a server, or turn on Include direct messages.".into()
        }
        CapError::Invalid { .. } if kind == Kind::Gmail => "Write the Gmail address.".into(),
        CapError::Provider { code, .. } if code == "gh_missing" => {
            "The gh command line tool is not installed here.".into()
        }
        CapError::Provider { code, .. }
            if matches!(code.as_str(), "slackcli_missing" | "discordcli_missing" | "not_installed")
                && !kind.missing_tool().is_empty() =>
        {
            kind.missing_tool().into()
        }
        CapError::Provider { code, message } if code == "account_mismatch" => {
            format!("{message}.")
        }
        other => format!("{} answered with a problem: {other}.", kind.name()),
    }
}

fn row_of(kind: Kind, error: &CapError) -> Row {
    match error {
        CapError::NotSignedIn => Row::NotSignedIn,
        CapError::Offline => Row::Offline,
        other => Row::Failed(plain_words(kind, other)),
    }
}

fn settle<P>(kind: Kind, connected: CapResult<(P, String)>) -> (Row, Option<P>) {
    match connected {
        Ok((provider, name)) => (Row::Connected(name), Some(provider)),
        Err(error) => (row_of(kind, &error), None),
    }
}

/// The row of a build that stopped on a panic, so one kind that breaks leaves the others alone.
fn stopped<P>(kind: Kind) -> (Row, Option<P>) {
    (Row::Failed(format!("{} stopped unexpectedly.", kind.name())), None)
}

/// The providers for what is saved, and the state of each row. Reads the keychain and asks the network and the command line
/// tools: never on the UI thread. A kind that cannot be built is left out and says why in its row. The kinds are asked at
/// the same time, so a slow Linear (or a `gmailcli` that takes ten seconds) does not hold the others back.
pub(crate) fn build(saved: &AccountsSaved, services: &AccountServices) -> Built {
    let (linear, github, slack, discord, gmail) = std::thread::scope(|scope| {
        let linear = scope.spawn(|| build_linear(saved, services));
        let github = scope.spawn(|| build_github(saved, services));
        let slack = scope.spawn(|| build_slack(saved, services));
        let discord = scope.spawn(|| build_discord(saved, services));
        let gmail = build_gmail(saved, services);
        (
            linear.join().unwrap_or_else(|_| stopped(Kind::Linear)),
            github.join().unwrap_or_else(|_| stopped(Kind::GithubIssues)),
            slack.join().unwrap_or_else(|_| stopped(Kind::Slack)),
            discord.join().unwrap_or_else(|_| stopped(Kind::Discord)),
            gmail,
        )
    });
    Built {
        rows: Rows {
            linear: linear.0,
            github: github.0,
            slack: slack.0,
            discord: discord.0,
            gmail: gmail.0,
        },
        providers: [linear.1, github.1].into_iter().flatten().collect(),
        messaging: [slack.1, discord.1].into_iter().flatten().collect(),
        mail: [gmail.1].into_iter().flatten().collect(),
    }
}

fn build_linear(
    saved: &AccountsSaved,
    services: &AccountServices,
) -> (Row, Option<Arc<dyn TasksProvider>>) {
    // The keychain is read only for a person who connected Linear, so no one else sees a keychain prompt.
    match &saved.linear {
        None => (Row::Off, None),
        Some(_) => match services.secrets.read(LINEAR_KEY) {
            Ok(Some(key)) => settle(Kind::Linear, connect_linear(services, &key)),
            Ok(None) => (Row::NotSignedIn, None),
            Err(why) => (
                Row::Failed(format!("The keychain could not be read: {why}")),
                None,
            ),
        },
    }
}

fn build_github(
    saved: &AccountsSaved,
    services: &AccountServices,
) -> (Row, Option<Arc<dyn TasksProvider>>) {
    match &saved.github_issues {
        None => (Row::Off, None),
        Some(github) => settle(Kind::GithubIssues, connect_github(services, &github.repo)),
    }
}

fn build_slack(
    saved: &AccountsSaved,
    services: &AccountServices,
) -> (Row, Option<Arc<dyn MessagingProvider>>) {
    match &saved.slack {
        None => (Row::Off, None),
        Some(slack) => settle(Kind::Slack, connect_slack(services, slack)),
    }
}

fn build_discord(
    saved: &AccountsSaved,
    services: &AccountServices,
) -> (Row, Option<Arc<dyn MessagingProvider>>) {
    match &saved.discord {
        None => (Row::Off, None),
        Some(discord) => settle(Kind::Discord, connect_discord(services, discord)),
    }
}

fn build_gmail(
    saved: &AccountsSaved,
    services: &AccountServices,
) -> (Row, Option<Arc<dyn MailProvider>>) {
    match &saved.gmail {
        None => (Row::Off, None),
        Some(gmail) => settle(Kind::Gmail, connect_gmail(services, gmail)),
    }
}

/// Builds the providers for `saved` on a background thread, then gives them to the hub and tells every screen that
/// reads it. The rows say "Checking" meanwhile. Call it at startup and after each change in Settings.
pub(crate) fn refresh(saved: AccountsSaved, cx: &mut App) {
    let Some(hub) = cx.try_global::<CapabilityHub>().cloned() else {
        return;
    };
    let services = services(cx);
    let turn = hub.checking(&saved);
    tell(cx);
    cx.spawn(async move |cx| {
        let built = cx
            .background_spawn(async move { build(&saved, &services) })
            .await;
        cx.update(|cx| {
            // A newer refresh began while this one asked the network: its answer is the one to keep.
            if hub.install(built, turn) {
                tell(cx);
            }
        });
    })
    .detach();
}

/// Wakes what watches the hub: the Tasks screens and the Settings page.
fn tell(cx: &mut App) {
    cx.update_global::<CapabilityHub, _>(|_, _| {});
}
