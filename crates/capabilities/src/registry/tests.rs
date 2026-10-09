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
