use std::sync::Arc;

use atelier_agents::session::Account;
use atelier_settings::secrets::{InMemory, OPENROUTER_KEY, Secrets};
use atelier_ui::{
    scale::px,
    theme::{Appearance, set_appearance},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::{KeyCheck, account_name_ok};
use crate::{
    providers::{self, Choice, ProviderServices},
    settings_pane::{AgentRow, SettingsPane},
};

const KEY: &str = "sk-or-v1-abcd1234";
const KEY_TAIL: &str = "1234";
const REFUSED: &str = "OpenRouter does not know this key";
const USUAL: &str = "default";
const TEAM: &str = "team";
const MAX_PLAN: &str = "max";

fn two_accounts() -> Result<Vec<Account>, String> {
    Ok(vec![
        Account { name: USUAL.into(), signed_in: true, plan: Some(MAX_PLAN.into()), email: None },
        Account { name: TEAM.into(), signed_in: false, plan: None, email: None },
    ])
}

fn key_works(_: &str) -> Result<(), String> {
    Ok(())
}

fn key_refused(_: &str) -> Result<(), String> {
    Err(REFUSED.into())
}

fn services(secrets: &Arc<InMemory>, check_key: fn(&str) -> Result<(), String>) -> ProviderServices {
    ProviderServices { secrets: secrets.clone(), check_key, accounts: two_accounts }
}

/// The Settings page on its Providers section, pressed open from the list, with `services` in place.
fn open(services: ProviderServices, cx: &mut TestAppContext) -> (Entity<SettingsPane>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_appearance(Appearance::Light, cx);
        cx.set_reduce_motion(true);
        cx.set_global(services);
    });
    let agents = vec![AgentRow { name: "Claude Code".into(), mark: None, models: Vec::new() }];
    let (pane, cx) = cx.add_window_view(move |_, cx| SettingsPane::new(&atelier_settings::Settings::default(), agents, cx));
    cx.simulate_resize(gpui_kit::size(px(900.), px(900.)));
    settle(cx);
    let entry = cx.debug_bounds("section-providers").expect("Providers is in the list").center();
    cx.simulate_click(entry, gpui_kit::Modifiers::default());
    settle(cx);
    (pane, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn the_section_lists_each_account_and_says_no_key_is_kept(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, cx) = open(services(&secrets, key_works), cx);

    let page = pane.read_with(cx, |pane, _| (pane.providers.accounts.clone(), pane.providers.key_tail.clone(), pane.providers.problem.clone()));

    assert_eq!(page, (two_accounts().ok(), None, None));
    assert!(cx.debug_bounds("default-provider").is_some(), "the default is drawn");
}

#[gpui_kit::test]
fn a_kept_key_shows_only_its_last_characters(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    secrets.write(OPENROUTER_KEY, KEY).unwrap();
    let (pane, cx) = open(services(&secrets, key_works), cx);

    assert_eq!(pane.read_with(cx, |pane, _| pane.providers.key_tail.clone()), Some(KEY_TAIL.into()));
}

#[gpui_kit::test]
fn keeping_a_key_puts_it_in_the_keychain_and_checks_it(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, cx) = open(services(&secrets, key_works), cx);

    pane.update(cx, |pane, cx| pane.keep_key(format!("  {KEY}\n"), cx));
    settle(cx);

    assert_eq!(secrets.read(OPENROUTER_KEY).unwrap().as_deref(), Some(KEY), "kept without the spaces around it");
    assert_eq!(pane.read_with(cx, |pane, _| pane.providers.check.clone()), KeyCheck::Works);
}

#[gpui_kit::test]
fn a_refused_key_says_why(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    secrets.write(OPENROUTER_KEY, KEY).unwrap();
    let (pane, cx) = open(services(&secrets, key_refused), cx);

    pane.update(cx, |pane, cx| pane.test_key(cx));
    settle(cx);

    assert_eq!(pane.read_with(cx, |pane, _| pane.providers.check.clone()), KeyCheck::Refused(REFUSED.into()));
}

#[gpui_kit::test]
fn the_chosen_default_is_what_new_sessions_start_on(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    let (pane, cx) = open(services(&secrets, key_works), cx);

    pane.update(cx, |pane, cx| pane.choose_default(Choice::OpenRouter, cx));

    assert_eq!(cx.update(|_, cx| providers::default_choice(cx)), Choice::OpenRouter);
}

#[gpui_kit::test]
fn forgetting_the_key_takes_new_sessions_back_to_the_usual_account(cx: &mut TestAppContext) {
    let secrets = Arc::new(InMemory::default());
    secrets.write(OPENROUTER_KEY, KEY).unwrap();
    let (pane, cx) = open(services(&secrets, key_works), cx);
    pane.update(cx, |pane, cx| pane.choose_default(Choice::OpenRouter, cx));

    pane.update(cx, |pane, cx| pane.forget_key(cx));

    assert_eq!(secrets.read(OPENROUTER_KEY).unwrap(), None);
    assert_eq!(cx.update(|_, cx| providers::default_choice(cx)), Choice::usual());
}

#[test]
fn an_account_name_must_make_a_plain_folder_name() {
    for good in ["work", "client-a", "team_2"] {
        assert!(account_name_ok(good), "{good}");
    }
    for bad in ["", "default", "a b", "../x", "x/y", "é"] {
        assert!(!account_name_ok(bad), "{bad:?}");
    }
}
