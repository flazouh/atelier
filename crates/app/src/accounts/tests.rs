use std::{io, sync::Arc};

use atelier_capabilities::{
    CapError, CapResult,
    tasks::{MemoryTasks, TasksProvider},
};
use atelier_settings::{
    AccountsSaved, GithubIssuesSaved, LinearSaved,
    secrets::{InMemory, LINEAR_KEY, Secrets},
};

use super::{AccountServices, Kind, Row, build, plain_words};

mod chat_mail;

const GOOD_KEY: &str = "lin_api_good";
const REFUSED_KEY: &str = "lin_api_revoked";

fn linear(key: &str) -> CapResult<Arc<dyn TasksProvider>> {
    match key {
        GOOD_KEY => Ok(Arc::new(MemoryTasks::new("acme"))),
        _ => Err(CapError::NotSignedIn),
    }
}

fn github(repo: &str) -> CapResult<Arc<dyn TasksProvider>> {
    match repo {
        "acme/web" => Ok(Arc::new(MemoryTasks::new("acme.web"))),
        "acme/offline" => Err(CapError::Offline),
        "acme/private" => Err(CapError::not_found("the repository")),
        _ => Err(CapError::invalid("repo")),
    }
}

fn services(secrets: &Arc<InMemory>) -> AccountServices {
    AccountServices {
        secrets: secrets.clone(),
        linear,
        github,
        ..AccountServices::isolated()
    }
}

fn with_linear() -> AccountsSaved {
    AccountsSaved {
        linear: Some(LinearSaved::default()),
        ..Default::default()
    }
}

fn with_github(repo: &str) -> AccountsSaved {
    AccountsSaved {
        github_issues: Some(GithubIssuesSaved {
            repo: repo.into(),
            person: None,
        }),
        ..Default::default()
    }
}

/// A keychain that fails the test when it is asked.
struct Untouched;

impl Secrets for Untouched {
    fn read(&self, _: &str) -> io::Result<Option<String>> {
        panic!("the keychain was read for a reader who connected nothing");
    }
    fn write(&self, _: &str, _: &str) -> io::Result<()> {
        unreachable!()
    }
    fn forget(&self, _: &str) -> io::Result<()> {
        unreachable!()
    }
}

#[test]
fn nothing_connected_builds_nothing_and_never_asks_the_keychain() {
    let services = AccountServices {
        secrets: Arc::new(Untouched),
        linear,
        github,
        ..AccountServices::isolated()
    };
    let built = build(&AccountsSaved::default(), &services);
    assert!(built.providers.is_empty());
    assert_eq!((built.rows.linear, built.rows.github), (Row::Off, Row::Off));
}

#[test]
fn a_good_key_builds_a_provider_and_the_row_names_the_person() {
    let secrets = Arc::new(InMemory::default());
    secrets.write(LINEAR_KEY, GOOD_KEY).unwrap();
    let built = build(&with_linear(), &services(&secrets));
    assert_eq!(built.providers.len(), 1);
    assert_eq!(built.rows.linear, Row::Connected("Me".into()));
}

#[test]
fn a_revoked_key_leaves_the_provider_out_and_the_row_says_not_signed_in() {
    let secrets = Arc::new(InMemory::default());
    secrets.write(LINEAR_KEY, REFUSED_KEY).unwrap();
    let built = build(&with_linear(), &services(&secrets));
    assert!(built.providers.is_empty());
    assert_eq!(built.rows.linear, Row::NotSignedIn);
}

#[test]
fn a_connected_account_with_no_key_in_the_keychain_is_not_signed_in() {
    let built = build(&with_linear(), &services(&Arc::new(InMemory::default())));
    assert_eq!(built.rows.linear, Row::NotSignedIn);
}

#[test]
fn no_network_leaves_the_provider_out_and_the_row_says_offline() {
    let built = build(
        &with_github("acme/offline"),
        &services(&Arc::new(InMemory::default())),
    );
    assert!(built.providers.is_empty());
    assert_eq!(built.rows.github, Row::Offline);
}

#[test]
fn one_bad_account_does_not_hide_the_good_one() {
    let secrets = Arc::new(InMemory::default());
    secrets.write(LINEAR_KEY, REFUSED_KEY).unwrap();
    let saved = AccountsSaved {
        github_issues: with_github("acme/web").github_issues,
        ..with_linear()
    };
    let built = build(&saved, &services(&secrets));
    assert_eq!(
        built
            .providers
            .iter()
            .map(|p| p.account().to_string())
            .collect::<Vec<_>>(),
        ["acme.web"]
    );
    assert_eq!(
        (
            built.rows.linear,
            matches!(built.rows.github, Row::Connected(_))
        ),
        (Row::NotSignedIn, true)
    );
}

#[test]
fn a_repository_gh_cannot_see_says_so_in_plain_words() {
    let built = build(
        &with_github("acme/private"),
        &services(&Arc::new(InMemory::default())),
    );
    assert_eq!(
        built.rows.github,
        Row::Failed("GitHub does not show this repository to your gh login.".into())
    );
}

#[test]
fn errors_come_in_words_a_person_can_act_on() {
    assert!(plain_words(Kind::Linear, &CapError::NotSignedIn).contains("does not accept this key"));
    assert!(plain_words(Kind::GithubIssues, &CapError::NotSignedIn).contains("gh auth login"));
    assert!(plain_words(Kind::Linear, &CapError::Offline).contains("could not be reached"));
    assert!(
        plain_words(
            Kind::Linear,
            &CapError::RateLimited {
                retry_after_ms: 1500
            }
        )
        .contains("2 s")
    );
    assert!(plain_words(Kind::GithubIssues, &CapError::invalid("repo")).contains("owner/repo"));
}
