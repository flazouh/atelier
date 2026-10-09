use std::sync::Arc;

use atelier_capabilities::{
    CapError, CapResult,
    tasks::{MemoryTasks, TasksProvider},
};
use atelier_settings::{
    Settings,
    secrets::{InMemory, LINEAR_KEY, Secrets},
};
use atelier_ui::{
    scale::px,
    theme::{Appearance, set_appearance},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::Checked;
use crate::{
    accounts::{AccountServices, Row},
    capability_hub::CapabilityHub,
    settings_pane::{AgentRow, Section, SettingsPane},
};

const KEY: &str = "lin_api_SECRET0123";
const REVOKED: &str = "lin_api_revoked";

fn linear(key: &str) -> CapResult<Arc<dyn TasksProvider>> {
    match key {
        KEY => Ok(Arc::new(MemoryTasks::new("acme"))),
        _ => Err(CapError::NotSignedIn),
    }
}

fn github(repo: &str) -> CapResult<Arc<dyn TasksProvider>> {
    match repo {
        "acme/web" => Ok(Arc::new(MemoryTasks::new("acme.web"))),
        _ => Err(CapError::invalid("repo")),
    }
}

fn open<'a>(
    secrets: &Arc<InMemory>,
    cx: &'a mut TestAppContext,
) -> (
    Entity<SettingsPane>,
    CapabilityHub,
    &'a mut VisualTestContext,
) {
    open_with(&Settings::default(), secrets, cx)
}

/// The Settings page on its Accounts section, pressed open from the list, over a hub of its own.
fn open_with<'a>(
    saved: &Settings,
    secrets: &Arc<InMemory>,
    cx: &'a mut TestAppContext,
) -> (
    Entity<SettingsPane>,
    CapabilityHub,
    &'a mut VisualTestContext,
) {
    let hub = CapabilityHub::detached();
    let kept = hub.clone();
    let services = AccountServices {
        secrets: secrets.clone(),
        linear,
        github, ..AccountServices::isolated()
};
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_appearance(Appearance::Light, cx);
        cx.set_reduce_motion(true);
        cx.set_global(services);
        cx.set_global(kept);
    });
    let saved = saved.clone();
    let agents = vec![AgentRow {
        name: "Claude Code".into(),
        mark: None,
        backend: "claude-code".into(),
    }];
    let (pane, cx) = cx.add_window_view(move |_, cx| SettingsPane::new(&saved, agents, cx));
    cx.simulate_resize(gpui_kit::size(px(900.), px(900.)));
    settle(cx);
    let entry = cx
        .debug_bounds("section-accounts")
        .expect("Accounts is in the list")
        .center();
    cx.simulate_click(entry, gpui_kit::Modifiers::default());
    settle(cx);
    (pane, hub, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..4 {
        cx.run_until_parked();
    }
}

fn press(name: &str, cx: &mut VisualTestContext) {
    let at = cx
        .update(|_, cx| crate::control::find(name, cx))
        .unwrap_or_else(|| panic!("{name} is not drawn"))
        .center();
    cx.update(|window, cx| crate::control::press_in_steps(window, at, cx));
    settle(cx);
}

fn linear_check(pane: &Entity<SettingsPane>, cx: &mut VisualTestContext) -> Checked {
    pane.read_with(cx, |p, _| p.accounts.linear_check.clone())
}

#[test]
fn accounts_is_listed_right_after_providers() {
    let at = Section::ALL
        .iter()
        .position(|s| *s == Section::Providers)
        .unwrap();
    assert_eq!(Section::ALL[at + 1], Section::Accounts);
}

#[gpui_kit::test]
fn the_section_offers_a_key_for_linear_and_a_repository_for_github(cx: &mut TestAppContext) {
    let (_pane, _hub, cx) = open(&Arc::new(InMemory::default()), cx);

    assert!(
        cx.update(|_, cx| crate::control::find("linear-add", cx))
            .is_some(),
        "no key yet: a button adds one"
    );
    assert!(
        cx.debug_bounds("github-repo").is_some()
            || cx
                .update(|_, cx| crate::control::find("github-test", cx))
                .is_some()
    );
    assert!(
        cx.update(|_, cx| crate::control::find("github-forget", cx))
            .is_none(),
        "nothing to forget yet"
    );
}

#[gpui_kit::test]
fn a_bogus_key_fails_the_test_in_plain_words_and_keeps_nothing(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, _hub, cx) = open(&secrets, cx);

    pane.update(cx, |p, cx| p.test_linear(Some(REVOKED.into()), cx));
    settle(cx);

    let Checked::Refused(words) = linear_check(&pane, cx) else {
        panic!("the bogus key was accepted")
    };
    assert!(words.contains("does not accept this key"), "{words}");
    assert_eq!(
        secrets.read(LINEAR_KEY).unwrap(),
        None,
        "a test keeps nothing"
    );
}

#[gpui_kit::test]
fn a_good_key_names_the_person_it_belongs_to(cx: &mut TestAppContext) {
    let (pane, _hub, cx) = open(&Arc::new(InMemory::default()), cx);

    pane.update(cx, |p, cx| p.test_linear(Some(format!(" {KEY}\n")), cx));
    settle(cx);

    assert_eq!(linear_check(&pane, cx), Checked::Works("Me".into()));
}

#[gpui_kit::test]
fn saving_a_key_keeps_it_in_the_keychain_only_and_connects_the_account(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, hub, cx) = open(&secrets, cx);

    pane.update(cx, |p, cx| p.save_linear(format!(" {KEY}\n"), cx));
    settle(cx);

    assert_eq!(
        secrets.read(LINEAR_KEY).unwrap().as_deref(),
        Some(KEY),
        "kept without the spaces around it"
    );
    let saved = pane.read_with(cx, |p, _| format!("{:?}", p.accounts.saved));
    assert!(
        !saved.contains(KEY) && saved.contains("Me"),
        "the settings hold the person's name and no key: {saved}"
    );
    assert_eq!(
        hub.accounts()
            .iter()
            .map(|p| p.account().to_string())
            .collect::<Vec<_>>(),
        ["acme"],
        "the provider is built"
    );
    assert_eq!(hub.rows().linear, Row::Connected("Me".into()));
}

#[gpui_kit::test]
fn a_refused_key_is_kept_but_its_row_says_not_signed_in_and_no_provider_is_built(
    cx: &mut TestAppContext,
) {
    let secrets = Arc::new(InMemory::default());
    let (pane, hub, cx) = open(&secrets, cx);

    pane.update(cx, |p, cx| p.save_linear(REVOKED.into(), cx));
    settle(cx);

    assert!(matches!(linear_check(&pane, cx), Checked::Refused(_)));
    assert!(hub.accounts().is_empty());
    assert_eq!(hub.rows().linear, Row::NotSignedIn);
}

#[gpui_kit::test]
fn forgetting_takes_the_key_and_the_provider_away(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, hub, cx) = open(&secrets, cx);
    pane.update(cx, |p, cx| p.save_linear(KEY.into(), cx));
    settle(cx);
    assert_eq!(hub.accounts().len(), 1);

    pane.update(cx, |p, cx| p.show(Section::Accounts, cx));
    settle(cx);
    press("linear-forget", cx);

    assert_eq!(secrets.read(LINEAR_KEY).unwrap(), None);
    assert!(pane.read_with(cx, |p, _| p.accounts.saved.linear.is_none()));
    assert!(hub.accounts().is_empty());
    assert_eq!(hub.rows().linear, Row::Off);
}

#[gpui_kit::test]
fn a_kept_key_is_shown_by_its_last_characters_when_the_page_opens(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    secrets.write(LINEAR_KEY, KEY).unwrap();
    let saved = Settings {
        accounts: atelier_settings::AccountsSaved {
            linear: Some(Default::default()),
            ..Default::default()
        },
        ..Default::default()
    };
    let (pane, _hub, cx) = open_with(&saved, &secrets, cx);

    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts.linear_tail.clone()),
        Some("0123".into())
    );
    assert!(
        cx.update(|_, cx| crate::control::find("linear-forget", cx))
            .is_some()
    );
}

#[gpui_kit::test]
fn a_repository_is_saved_and_its_provider_built_and_forgetting_leaves_it_out(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = open(&Arc::new(InMemory::default()), cx);

    pane.update(cx, |p, cx| p.save_github(" acme/web ".into(), cx));
    settle(cx);

    assert_eq!(
        pane.read_with(cx, |p, _| p
            .accounts
            .saved
            .github_issues
            .as_ref()
            .map(|g| g.repo.clone())),
        Some("acme/web".into())
    );
    assert_eq!(hub.rows().github, Row::Connected("Me".into()));
    assert_eq!(hub.accounts().len(), 1);

    pane.update(cx, |p, cx| p.show(Section::Accounts, cx));
    settle(cx);
    press("github-forget", cx);

    assert!(pane.read_with(cx, |p, _| p.accounts.saved.github_issues.is_none()));
    assert!(hub.accounts().is_empty());
}

#[gpui_kit::test]
fn a_repository_that_is_not_owner_slash_repo_says_how_to_write_it(cx: &mut TestAppContext) {
    let (pane, hub, cx) = open(&Arc::new(InMemory::default()), cx);

    pane.update(cx, |p, cx| p.test_github("nonsense".into(), cx));
    settle(cx);

    let check = pane.read_with(cx, |p, _| p.accounts.github_check.clone());
    assert_eq!(
        check,
        Checked::Refused("Write the repository as owner/repo.".into())
    );
    assert!(
        hub.accounts().is_empty(),
        "a test builds nothing for the screens"
    );
}

#[gpui_kit::test]
fn the_projects_repository_is_offered_and_fills_the_field(cx: &mut TestAppContext) {
    let (pane, _hub, cx) = open(&Arc::new(InMemory::default()), cx);
    assert!(
        cx.update(|_, cx| crate::control::find("github-use-project", cx))
            .is_none(),
        "no repository to offer"
    );

    pane.update(cx, |p, cx| p.set_project_repo(Some("acme/web".into()), cx));
    settle(cx);
    press("github-use-project", cx);
    let typed = pane.read_with(cx, |p, cx| {
        p.accounts
            .github_field
            .as_ref()
            .map(|f| f.read(cx).value().to_string())
    });
    assert_eq!(
        typed.as_deref(),
        Some("acme/web"),
        "the field holds the project's repository"
    );
    press("github-save", cx);

    assert_eq!(
        pane.read_with(cx, |p, _| p
            .accounts
            .saved
            .github_issues
            .as_ref()
            .map(|g| g.repo.clone())),
        Some("acme/web".into())
    );
}

/// A keychain that is there but refuses to keep anything.
struct Locked;

impl Secrets for Locked {
    fn read(&self, _: &str) -> std::io::Result<Option<String>> {
        Ok(None)
    }
    fn write(&self, _: &str, _: &str) -> std::io::Result<()> {
        Err(std::io::Error::other("the keychain is locked"))
    }
    fn forget(&self, _: &str) -> std::io::Result<()> {
        Ok(())
    }
}

#[gpui_kit::test]
fn a_keychain_that_will_not_keep_the_key_is_said_in_words_and_nothing_is_connected_until_the_next_try_clears_it(
    cx: &mut TestAppContext,
) {
    let (pane, hub, cx) = open(&Arc::new(InMemory::default()), cx);
    cx.update(|_, cx| {
        cx.set_global(AccountServices {
            secrets: Arc::new(Locked),
            linear,
            github, ..AccountServices::isolated()
})
    });

    pane.update(cx, |p, cx| p.save_linear(KEY.into(), cx));
    settle(cx);

    let problem = pane.read_with(cx, |p, _| p.accounts.problem.clone());
    assert_eq!(
        problem.as_deref(),
        Some("The keychain could not keep the key: the keychain is locked")
    );
    assert!(
        pane.read_with(cx, |p, _| p.accounts.saved.linear.is_none()),
        "not connected: the key is not kept"
    );
    assert!(hub.accounts().is_empty());

    pane.update(cx, |p, cx| p.test_linear(Some(KEY.into()), cx));
    settle(cx);
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts.problem.clone()),
        None,
        "an old complaint does not outlive the next try"
    );
}
