use atelier_agents::session::{Account, ApiKey, Provider};
use atelier_settings::secrets::{InMemory, OPENROUTER_KEY, Secrets};

use super::{Choice, NO_KEY, label, provider, row_words};

const WORK: &str = "work";
const USUAL: &str = "default";
const KEY: &str = "sk-or-v1-test";
const MAX_PLAN: &str = "max";

fn account(name: &str, plan: Option<&str>) -> Account {
    Account { name: name.into(), signed_in: plan.is_some(), plan: plan.map(str::to_string), email: None }
}

#[test]
fn a_choice_is_kept_as_a_key_and_read_back() {
    for choice in [Choice::Account(USUAL.into()), Choice::Account(WORK.into()), Choice::OpenRouter] {
        assert_eq!(Choice::from_key(&choice.key()), Some(choice.clone()), "{choice:?}");
    }
    assert_eq!(Choice::from_key("something else"), None);
}

#[test]
fn nothing_saved_is_the_usual_account() {
    assert_eq!(Choice::saved(None), Choice::Account(USUAL.into()));
    assert_eq!(Choice::saved(Some("garbage")), Choice::Account(USUAL.into()));
    assert_eq!(Choice::saved(Some(&Choice::OpenRouter.key())), Choice::OpenRouter);
}

#[test]
fn an_account_becomes_its_provider_without_a_key() {
    let provider = provider(&Choice::Account(WORK.into()), &InMemory::default());

    assert_eq!(provider, Ok(Provider::Account(WORK.into())));
}

#[test]
fn openrouter_takes_the_key_from_the_keychain() {
    let secrets = InMemory::default();
    secrets.write(OPENROUTER_KEY, KEY).unwrap();

    assert_eq!(provider(&Choice::OpenRouter, &secrets), Ok(Provider::OpenRouter { key: ApiKey::new(KEY) }));
}

#[test]
fn openrouter_with_no_key_says_where_to_add_one() {
    assert_eq!(provider(&Choice::OpenRouter, &InMemory::default()), Err(NO_KEY.to_string()));
}

#[test]
fn an_account_is_named_by_its_plan_and_its_name() {
    let accounts = [account(USUAL, Some(MAX_PLAN)), account(WORK, Some(MAX_PLAN))];

    assert_eq!(label(&Choice::Account(USUAL.into()), &accounts), "Max");
    assert_eq!(label(&Choice::Account(WORK.into()), &accounts), "Max · work");
    assert_eq!(label(&Choice::Account(WORK.into()), &[]), "work", "an account not read yet");
    assert_eq!(label(&Choice::Account(USUAL.into()), &[]), "Claude");
    assert_eq!(label(&Choice::OpenRouter, &accounts), "OpenRouter");
}

#[test]
fn a_session_row_names_every_provider_but_the_usual_account() {
    assert_eq!(row_words(&Choice::Account(USUAL.into())), None);
    assert_eq!(row_words(&Choice::Account(WORK.into())).as_deref(), Some(WORK));
    assert_eq!(row_words(&Choice::OpenRouter).as_deref(), Some("OpenRouter"));
}
