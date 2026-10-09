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
