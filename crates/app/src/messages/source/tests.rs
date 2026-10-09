use std::sync::Arc;

use atelier_capabilities::messaging::{MemoryMessaging, MessagingProvider};

use super::*;

fn memory(account: &str) -> Arc<dyn MessagingProvider> {
    Arc::new(MemoryMessaging::new(account))
}

#[test]
fn a_source_lists_its_accounts_in_the_registrys_order() {
    let source = MessagesSource::from_providers([memory("beta"), memory("alpha")]);
    let accounts = source.accounts();
    let words: Vec<_> = accounts
        .iter()
        .map(|(choice, _)| choice.words().to_string())
        .collect();
    assert_eq!(words, ["Memory · alpha", "Memory · beta"]);
    assert_eq!(
        accounts[1].1.account(),
        "beta",
        "each choice goes with its provider"
    );
}

#[test]
fn a_source_with_no_account_is_empty() {
    let source = MessagesSource::from_providers([]);
    assert!(source.accounts().is_empty());
}

#[test]
fn an_account_added_twice_is_listed_once() {
    let source = MessagesSource::from_providers([memory("a"), memory("a")]);
    assert_eq!(source.accounts().len(), 1);
}

#[test]
fn the_same_accounts_are_the_same_source() {
    let a = memory("a");
    let b = memory("b");
    let first = MessagesSource::from_providers([a.clone(), b.clone()]);
    assert!(
        first.same_as(&[b.clone(), a.clone()]),
        "order does not matter"
    );
    assert!(!first.same_as(std::slice::from_ref(&a)), "an account fewer");
    assert!(
        !first.same_as(&[a, memory("b")]),
        "another provider of the same name is another account"
    );
}

#[test]
fn a_choice_reads_as_a_person_would_say_it() {
    let choice = Choice {
        provider: "slack".into(),
        account: "acme".into(),
    };
    assert_eq!(choice.name(), "Slack");
    assert_eq!(choice.words(), "Slack · acme");
}
