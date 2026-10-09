use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError, CapResult,
    tasks::{Query, TasksProvider},
};
use atelier_settings::{AccountsSaved, secrets::LINEAR_KEY};
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
        CapError::Invalid { .. } if kind == Kind::GithubIssues => {
            "Write the repository as owner/repo.".into()
        }
        CapError::Provider { code, .. } if code == "gh_missing" => {
            "The gh command line tool is not installed here.".into()
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

fn settle(
    kind: Kind,
    connected: CapResult<(Arc<dyn TasksProvider>, String)>,
) -> (Row, Option<Arc<dyn TasksProvider>>) {
    match connected {
        Ok((provider, name)) => (Row::Connected(name), Some(provider)),
        Err(error) => (row_of(kind, &error), None),
    }
}

/// The providers for what is saved, and the state of each row. Reads the keychain and asks the network: never on the UI
/// thread. A kind that cannot be built is left out and says why in its row. The two kinds are asked at the same time, so
/// a slow Linear does not hold GitHub back.
pub(crate) fn build(saved: &AccountsSaved, services: &AccountServices) -> Built {
    let (linear, github) = std::thread::scope(|scope| {
        let linear = scope.spawn(|| build_linear(saved, services));
        let github = build_github(saved, services);
        (
            linear
                .join()
                .unwrap_or_else(|_| (Row::Failed("Linear stopped unexpectedly.".into()), None)),
            github,
        )
    });
    let providers = [linear.1, github.1].into_iter().flatten().collect();
    Built {
        rows: Rows {
            linear: linear.0,
            github: github.0,
        },
        providers,
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
