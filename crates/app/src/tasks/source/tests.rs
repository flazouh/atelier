use std::sync::Arc;

use atelier_capabilities::{
    Operation, Ref,
    tasks::{Category, Label, MemoryTasks, Status, TasksProvider},
};
use atelier_ui::task_model::TaskStatus;

use super::*;

fn memory(account: &str) -> Arc<dyn TasksProvider> {
    Arc::new(MemoryTasks::new(account))
}

#[test]
fn a_source_lists_its_providers_and_shows_the_first() {
    let source = TasksSource::from_providers([memory("beta"), memory("alpha")]);
    let words: Vec<_> = source.choices().iter().map(|c| c.words().to_string()).collect();
    assert_eq!(words, ["Memory · alpha", "Memory · beta"], "in the registry's order");
    assert_eq!(source.provider().map(|p| p.account().to_string()), Some("beta".to_string()), "the first one added is shown");
    assert_eq!(source.selected(), Some(1));
}

#[test]
fn choosing_a_provider_shows_it_and_a_missing_one_changes_nothing() {
    let mut source = TasksSource::from_providers([memory("alpha"), memory("beta")]);
    assert!(source.select(1));
    assert_eq!(source.provider().map(|p| p.account().to_string()), Some("beta".to_string()));
    assert!(!source.select(5));
    assert_eq!(source.selected(), Some(1));
}

#[test]
fn a_source_with_no_provider_shows_none() {
    let source = TasksSource::from_providers([]);
    assert!(source.provider().is_none() && source.choices().is_empty() && source.selected().is_none());
}

#[test]
fn a_provider_without_custom_states_has_the_six_plain_statuses() {
    let vocab = Vocabulary::default();
    for status in [TaskStatus::Backlog, TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::InReview, TaskStatus::Done, TaskStatus::Canceled] {
        assert!(vocab.status_id(status).is_some(), "{status:?}");
    }
}

#[test]
fn a_status_is_the_first_of_the_provider_in_its_category_and_a_missing_one_is_none() {
    let ready = Status { id: "ready".into(), name: "Ready for QA".into(), category: Category::InReview };
    let shipped = Status { id: "shipped".into(), name: "Shipped".into(), category: Category::Done };
    let vocab = Vocabulary::new("linear", "acme", Default::default(), vec![ready, shipped, Status::plain(Category::Todo)], vec![], vec![]);
    assert_eq!(vocab.status_id(TaskStatus::InReview).as_deref(), Some("ready"));
    assert_eq!(vocab.status_id(TaskStatus::Done).as_deref(), Some("shipped"));
    assert_eq!(vocab.status_id(TaskStatus::Backlog), None);
}

#[test]
fn a_call_that_was_listed_can_be_taken_back() {
    let mut vocab = Vocabulary::default();
    assert!(vocab.can(Operation::Create));
    vocab.disable(Operation::Create);
    assert!(!vocab.can(Operation::Create) && vocab.can(Operation::Update));
}

#[test]
fn a_reference_shows_its_name_and_one_the_lists_lack_shows_its_id() {
    let bug = Ref::new("tasks", "linear", "acme", "lbl_1").unwrap();
    let label = Label { reference: bug.clone(), name: "bug".into(), color: None, raw: None };
    let vocab = Vocabulary::new("linear", "acme", Default::default(), vec![], vec![label], vec![]);
    assert_eq!(vocab.name_of(&bug), "bug");
    assert_eq!(vocab.label_ref("bug"), bug);
    let unknown = Ref::new("tasks", "linear", "acme", "lbl_9").unwrap();
    assert_eq!(vocab.name_of(&unknown), "lbl_9");
    assert_eq!(vocab.label_ref("new").to_string(), "tasks:linear:acme:new", "a new name takes the provider's own form");
}

#[test]
fn the_switcher_follows_the_connected_accounts() {
    let mut source = TasksSource::from_providers([memory("project")]);
    assert_eq!(source.choices().len(), 1, "one provider: no switcher");

    source.set_accounts(vec![memory("acme"), memory("web")]);
    assert_eq!(source.choices().len(), 3);

    source.set_accounts(vec![memory("acme")]);
    assert_eq!(source.choices().len(), 2, "an account taken out leaves the list");

    source.set_accounts(Vec::new());
    assert_eq!(source.choices().len(), 1);
}

#[test]
fn the_shown_account_going_away_shows_the_first_provider_left() {
    let mut source = TasksSource::from_providers([memory("project")]);
    source.set_accounts(vec![memory("acme")]);
    let at = source.choices().iter().position(|c| c.account == "acme").unwrap();
    assert!(source.select(at));

    source.set_accounts(Vec::new());

    assert_eq!(source.provider().map(|p| p.account().to_string()), Some("project".to_string()));
}

#[test]
fn an_account_built_again_replaces_the_one_held() {
    let mut source = TasksSource::from_providers([memory("project")]);
    let first = memory("acme");
    source.set_accounts(vec![first.clone()]);
    let second = memory("acme");

    source.set_accounts(vec![second.clone()]);

    let at = source.choices().iter().position(|c| c.account == "acme").unwrap();
    assert!(source.select(at));
    assert!(std::ptr::addr_eq(Arc::as_ptr(&source.provider().unwrap()), Arc::as_ptr(&second)));
}
