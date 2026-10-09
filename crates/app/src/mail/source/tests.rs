use std::sync::Arc;

use atelier_capabilities::mail::{MailProvider, MemoryMail};

use super::*;

fn memory(account: &str) -> Arc<dyn MailProvider> {
    Arc::new(MemoryMail::new(account))
}

#[test]
fn a_source_lists_its_accounts_in_the_registrys_order() {
    let source = MailSource::from_providers([memory("b@x.test"), memory("a@x.test")]);
    let accounts = source.accounts();
    let words: Vec<_> = accounts
        .iter()
        .map(|(choice, _)| choice.words().to_string())
        .collect();
    assert_eq!(words, ["Memory · a@x.test", "Memory · b@x.test"]);
    assert_eq!(
        accounts[1].1.account(),
        "b@x.test",
        "each choice goes with its provider"
    );
}

#[test]
fn a_source_with_no_account_is_empty() {
    assert!(MailSource::from_providers([]).accounts().is_empty());
}

#[test]
fn an_account_added_twice_is_listed_once() {
    let source = MailSource::from_providers([memory("a@x.test"), memory("a@x.test")]);
    assert_eq!(source.accounts().len(), 1);
}

#[test]
fn the_same_accounts_are_the_same_source() {
    let (a, b) = (memory("a@x.test"), memory("b@x.test"));
    let first = MailSource::from_providers([a.clone(), b.clone()]);
    assert!(
        first.same_as(&[b.clone(), a.clone()]),
        "order does not matter"
    );
    assert!(!first.same_as(std::slice::from_ref(&a)), "an account fewer");
    assert!(
        !first.same_as(&[a, memory("b@x.test")]),
        "another provider of the same name is another account"
    );
}

#[test]
fn a_choice_reads_as_a_person_would_say_it() {
    let choice = Choice {
        provider: "gmail".into(),
        account: "alex@example.com".into(),
    };
    assert_eq!(choice.name(), "Gmail");
    assert_eq!(choice.words(), "Gmail · alex@example.com");
}
