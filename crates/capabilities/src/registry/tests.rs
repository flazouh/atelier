use super::*;
use crate::tasks::MemoryTasks;

#[test]
fn providers_are_found_by_provider_and_account() {
    let mut r = Registry::new();
    r.add_tasks(Arc::new(MemoryTasks::new("a")));
    r.add_tasks(Arc::new(MemoryTasks::new("b")));
    assert_eq!(r.all_tasks().len(), 2);
    assert_eq!(r.tasks("memory", "b").unwrap().account(), "b");
    assert!(r.tasks("memory", "c").is_none() && r.tasks("linear", "a").is_none());
}

#[test]
fn a_second_provider_for_the_same_account_replaces_the_first() {
    let mut r = Registry::new();
    r.add_tasks(Arc::new(MemoryTasks::new("a")));
    r.add_tasks(Arc::new(MemoryTasks::new("a")));
    assert_eq!(r.all_tasks().len(), 1);
}

#[test]
fn messaging_providers_are_found_by_provider_and_account() {
    let mut r = Registry::new();
    r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("a")));
    r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("a")));
    assert_eq!(r.all_messaging().len(), 1);
    assert!(r.messaging("memory", "a").is_some() && r.messaging("slack", "a").is_none());
}

#[test]
fn mail_providers_are_found_by_provider_and_account() {
    let mut r = Registry::new();
    r.add_mail(Arc::new(crate::mail::MemoryMail::new("a")));
    r.add_mail(Arc::new(crate::mail::MemoryMail::new("a")));
    assert_eq!(r.all_mail().len(), 1);
    assert!(r.mail("memory", "a").is_some() && r.mail("gmail", "a").is_none());
}

#[test]
fn a_removed_tasks_provider_is_gone_and_the_others_stay() {
    let mut r = Registry::new();
    r.add_tasks(Arc::new(MemoryTasks::new("a")));
    r.add_tasks(Arc::new(MemoryTasks::new("b")));
    assert_eq!(
        r.remove_tasks("memory", "a")
            .map(|p| p.account().to_string()),
        Some("a".into())
    );
    assert!(r.tasks("memory", "a").is_none() && r.tasks("memory", "b").is_some());
    assert!(
        r.remove_tasks("memory", "a").is_none(),
        "removing twice finds nothing"
    );
}

#[test]
fn a_messaging_or_mail_account_can_be_taken_out_and_only_that_one() {
    let mut r = Registry::new();
    r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("a")));
    r.add_messaging(Arc::new(crate::messaging::MemoryMessaging::new("b")));
    r.add_mail(Arc::new(crate::mail::MemoryMail::new("a")));
    assert!(r.remove_messaging("memory", "a").is_some());
    assert!(
        r.remove_messaging("memory", "a").is_none(),
        "nothing left to take"
    );
    assert_eq!(r.all_messaging().len(), 1);
    assert!(r.remove_mail("memory", "a").is_some());
    assert!(r.all_mail().is_empty());
}
