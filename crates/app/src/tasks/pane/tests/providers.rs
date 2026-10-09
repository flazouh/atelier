//! What the screen does with what a provider says it can do, with the errors a provider answers with, with pages
//! and with more than one provider.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use atelier_capabilities::{
    CapError, Operation, Ref,
    tasks::{Category, NewTask, Patch, Priority, Query, TasksProvider},
};
use atelier_ui::task_model::TaskStatus;
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::fake::Fake;
use super::{open_with, settle};
use crate::tasks::pane::{Change, Load, Problem, Reaction, Scope, Source, TasksEvent, TasksPane, apply, react};
use crate::tasks::source::TasksSource;

fn source_of(providers: &[&Arc<Fake>]) -> TasksSource {
    TasksSource::from_providers(providers.iter().map(|p| (*p).clone() as Arc<dyn TasksProvider>))
}

fn press(name: &'static str, pane: &Entity<TasksPane>, cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn")).center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(pane, cx);
}

fn draft(title: &str) -> atelier_ui::new_task_model::Draft {
    atelier_ui::new_task_model::Draft { title: title.into(), ..Default::default() }
}

fn ids(pane: &Entity<TasksPane>, cx: &mut VisualTestContext) -> Vec<gpui_kit::SharedString> {
    pane.read_with(cx, |p, _| p.tasks().iter().map(|t| t.id.clone()).collect())
}

fn titles(pane: &Entity<TasksPane>, cx: &mut VisualTestContext) -> Vec<String> {
    pane.read_with(cx, |p, _| p.tasks().iter().map(|t| t.title.to_string()).collect())
}

#[test]
fn each_error_has_one_answer() {
    assert_eq!(react(&CapError::Offline), Reaction::Raise(Problem::Offline));
    assert_eq!(react(&CapError::NotSignedIn), Reaction::Raise(Problem::SignedOut));
    assert_eq!(react(&CapError::RateLimited { retry_after_ms: 1500 }), Reaction::Raise(Problem::Wait(1500)));
    assert_eq!(react(&CapError::unsupported("create")), Reaction::Hide);
    for other in [CapError::not_found("x"), CapError::invalid("title"), CapError::Storage { message: "disk".into() }, CapError::Provider { code: "E1".into(), message: "boom".into() }] {
        assert!(matches!(react(&other), Reaction::Line(words) if words == other.to_string()), "{other:?} is one line");
    }
}

#[test]
fn a_change_of_a_task_that_moved_on_is_sent_again_against_the_version_now() {
    let provider = Fake::seeded("a", &["One"]);
    let me = atelier_capabilities::Actor::person("me", "me");
    let task = provider.memory().list(&Query::default()).unwrap().items.remove(0);
    provider.memory().update(&task.reference, &Patch { priority: Some(Priority::High), ..Patch::default() }, &task.version, &me).unwrap();
    let patch = Patch { status: Some("done".into()), ..Patch::default() };
    let made = apply(provider.as_ref(), &task.reference, &patch, &task.version, &me).expect("the stale version is read again");
    assert_eq!((made.status.category, made.priority), (Category::Done, Priority::High), "both changes stand");
}

#[gpui_kit::test]
fn a_provider_without_create_has_no_way_to_make_a_task(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.without(Operation::Create);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-new").is_none(), "no New task button");
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    assert!(!pane.read_with(cx, |p, _| p.creating), "the key opens no dialog either");
}

#[gpui_kit::test]
fn a_provider_with_create_shows_the_control(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-new").is_some());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    assert!(pane.read_with(cx, |p, _| p.creating));
}

#[gpui_kit::test]
fn a_provider_without_labels_shows_no_labels(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &[]);
    let bug = Ref::new("tasks", "memory", "a", "bug").unwrap();
    let me = atelier_capabilities::Actor::person("me", "me");
    provider.memory().create(&NewTask { title: "Fix".into(), labels: vec![bug], ..NewTask::default() }, &me).unwrap();
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert_eq!(pane.read_with(cx, |p, _| p.labels().iter().map(|l| l.name.to_string()).collect::<Vec<_>>()), ["bug"], "listed, so shown");
    provider.without(Operation::Labels);
    pane.update(cx, |p, cx| p.reload(cx));
    settle(&pane, cx);
    assert!(pane.read_with(cx, |p, _| p.labels().is_empty()), "not listed, so hidden");
}

#[gpui_kit::test]
fn a_status_the_provider_lacks_is_refused_and_one_it_has_is_kept(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.statuses_only(&[Category::Todo, Category::Done]);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    let id = ids(&pane, cx);
    pane.update(cx, |p, cx| p.changed(&id, &Change::Status(TaskStatus::InReview), Source::None, cx));
    settle(&pane, cx);
    let held = || super::held(provider.as_ref());
    assert_eq!(held()[0].1, Category::Todo, "nothing was sent");
    let said = pane.read_with(cx, |p, _| p.said.clone()).expect("it says why");
    assert!(said.contains("In Review"), "{said}");
    pane.update(cx, |p, cx| p.changed(&id, &Change::Status(TaskStatus::Done), Source::None, cx));
    settle(&pane, cx);
    assert_eq!(held()[0].1, Category::Done);
}

#[gpui_kit::test]
fn a_provider_that_does_not_list_update_takes_no_change(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.without(Operation::Update);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    let id = ids(&pane, cx);
    pane.update(cx, |p, cx| p.changed(&id, &Change::Status(TaskStatus::Done), Source::None, cx));
    settle(&pane, cx);
    assert_eq!(super::held(provider.as_ref())[0].1, Category::Todo);
    assert!(pane.read_with(cx, |p, _| p.said.is_some()));
}

#[gpui_kit::test]
fn two_quick_changes_of_one_task_both_land(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    let id = ids(&pane, cx);
    pane.update(cx, |p, cx| {
        p.changed(&id, &Change::Status(TaskStatus::Done), Source::None, cx);
        p.changed(&id, &Change::Priority(atelier_ui::task_model::Priority::Urgent), Source::None, cx);
    });
    settle(&pane, cx);
    let task = provider.memory().list(&Query::default()).unwrap().items.remove(0);
    assert_eq!((task.status.category, task.priority), (Category::Done, Priority::Urgent));
    assert!(pane.read_with(cx, |p, _| p.said.is_none()), "no conflict reached the reader");
}

#[gpui_kit::test]
fn offline_shows_a_banner_with_retry_over_the_tasks_and_retry_reads_again(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::List, CapError::Offline);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert_eq!(pane.read_with(cx, |p, _| p.problem), Some(Problem::Offline));
    assert!(cx.debug_bounds("tasks-banner").is_some() && cx.debug_bounds("tasks-retry").is_some());
    assert!(titles(&pane, cx).is_empty());
    provider.heal(Operation::List);
    press("tasks-retry", &pane, cx);
    assert_eq!(titles(&pane, cx), ["One"]);
    assert!(cx.debug_bounds("tasks-banner").is_none(), "the banner went with the problem");
}

#[gpui_kit::test]
fn offline_after_the_tasks_were_read_keeps_them(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    provider.fail(Operation::List, CapError::Offline);
    pane.update(cx, |p, cx| p.reload(cx));
    settle(&pane, cx);
    assert_eq!(titles(&pane, cx), ["One"], "what was read stays");
    assert!(cx.debug_bounds("tasks-banner").is_some());
}

#[gpui_kit::test]
fn signed_out_shows_an_empty_state_with_a_button_that_asks_for_settings(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::List, CapError::NotSignedIn);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-signed-out").is_some() && cx.debug_bounds("tasks-sign-in").is_some());
    assert!(cx.debug_bounds("tasks-retry").is_none());
    let asked = Arc::new(AtomicBool::new(false));
    let seen = asked.clone();
    cx.update(|_, cx| {
        cx.subscribe(&pane, move |_, event: &TasksEvent, _| seen.store(matches!(event, TasksEvent::OpenAccounts), Ordering::SeqCst)).detach();
    });
    press("tasks-sign-in", &pane, cx);
    assert!(asked.load(Ordering::SeqCst), "the pane asked for Settings");
}

#[gpui_kit::test]
fn rate_limited_shows_the_wait_and_no_retry(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::List, CapError::RateLimited { retry_after_ms: 30_000 });
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert_eq!(pane.read_with(cx, |p, _| p.problem), Some(Problem::Wait(30_000)));
    assert!(cx.debug_bounds("tasks-banner").is_some());
    assert!(cx.debug_bounds("tasks-retry").is_none());
}

#[gpui_kit::test]
fn a_call_that_is_unsupported_loses_its_control_and_says_nothing(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::Create, CapError::unsupported("create a task"));
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-new").is_some(), "listed, so offered");
    cx.update(|window, cx| pane.update(cx, |p, cx| p.create(&draft("Two"), false, window, cx)));
    settle(&pane, cx);
    assert!(cx.debug_bounds("tasks-new").is_none(), "answered Unsupported, so gone");
    assert!(pane.read_with(cx, |p, _| p.said.is_none() && p.problem.is_none()), "no error line and no banner");
}

#[gpui_kit::test]
fn any_other_error_is_one_line_in_the_pane(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::Update, CapError::Provider { code: "E1".into(), message: "boom".into() });
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    let id = ids(&pane, cx);
    pane.update(cx, |p, cx| p.changed(&id, &Change::Status(TaskStatus::Done), Source::None, cx));
    settle(&pane, cx);
    let said = pane.read_with(cx, |p, _| p.said.clone()).expect("a line");
    assert!(said.contains("boom") && said.starts_with("Could not save"), "{said}");
    assert!(cx.debug_bounds("tasks-banner").is_none() && cx.debug_bounds("tasks-signed-out").is_none());
}

#[gpui_kit::test]
fn a_list_that_cannot_be_read_says_why_in_place_of_the_tasks(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::List, CapError::Provider { code: "E2".into(), message: "bad".into() });
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(pane.read_with(cx, |p, _| matches!(&p.load, Load::Failed(words) if words.contains("bad"))));
}

#[gpui_kit::test]
fn a_page_with_a_next_cursor_shows_load_more_and_each_press_adds_a_page(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["T1", "T2", "T3", "T4", "T5"]);
    provider.pages_of(2);
    let (pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert_eq!(titles(&pane, cx).len(), 2, "the first page");
    assert!(cx.debug_bounds("tasks-load-more").is_some());
    press("tasks-load-more", &pane, cx);
    assert_eq!(titles(&pane, cx).len(), 4);
    pane.update(cx, |p, cx| p.reload(cx));
    settle(&pane, cx);
    assert_eq!(titles(&pane, cx).len(), 4, "a reading after a change keeps the pages the reader loaded");
    press("tasks-load-more", &pane, cx);
    let all = titles(&pane, cx);
    assert_eq!(all.len(), 5, "the last page");
    assert_eq!(all.iter().collect::<std::collections::BTreeSet<_>>().len(), 5, "no task twice");
    assert!(cx.debug_bounds("tasks-load-more").is_none(), "no cursor, no row");
}

#[gpui_kit::test]
fn a_provider_that_fits_in_one_page_has_no_load_more(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["T1", "T2"]);
    let (_pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-load-more").is_none());
}

#[gpui_kit::test]
fn the_switcher_shows_with_two_providers_and_reads_the_one_chosen(cx: &mut TestAppContext) {
    let (a, b) = (Fake::seeded("alpha", &["From alpha"]), Fake::seeded("beta", &["From beta"]));
    let (pane, cx) = open_with(source_of(&[&a, &b]), 900., cx);
    assert!(cx.debug_bounds("tasks-provider").is_some(), "two providers, so a switcher");
    assert_eq!(titles(&pane, cx), ["From alpha"]);
    pane.update(cx, |p, cx| p.choose(1, cx));
    settle(&pane, cx);
    assert_eq!(titles(&pane, cx), ["From beta"], "the second provider's tasks, and not the first's");
}

#[gpui_kit::test]
fn the_switcher_is_not_there_with_one_provider(cx: &mut TestAppContext) {
    let a = Fake::seeded("alpha", &["From alpha"]);
    let (_pane, cx) = open_with(source_of(&[&a]), 900., cx);
    assert!(cx.debug_bounds("tasks-provider").is_none());
}

#[gpui_kit::test]
fn a_task_asked_for_before_the_tasks_are_read_opens_when_they_are(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
    });
    let provider = Fake::seeded("a", &["One", "Two"]);
    let (pane, cx) = cx.add_window_view(|window, cx| TasksPane::new("me", Vec::new(), window, cx));
    cx.update(|window, cx| pane.update(cx, |p, cx| p.show_key("MEM-2", window, cx)));
    assert!(pane.read_with(cx, |p, _| p.open.is_none()), "nothing to show yet");
    pane.update(cx, |p, cx| p.attach(Ok(source_of(&[&provider])), cx));
    settle(&pane, cx);
    let open = pane.read_with(cx, |p, _| p.open.clone()).expect("the task is shown");
    assert_eq!(pane.read_with(cx, |p, _| p.tasks().iter().find(|t| t.id == open).map(|t| t.title.to_string())).as_deref(), Some("Two"));
}

#[gpui_kit::test]
fn choosing_another_provider_shows_all_of_its_tasks_not_the_scope_of_the_last(cx: &mut TestAppContext) {
    let (a, b) = (Fake::seeded("alpha", &["One"]), Fake::seeded("beta", &["Two"]));
    let (pane, cx) = open_with(source_of(&[&a, &b]), 900., cx);
    pane.update_in(cx, |p, window, cx| p.set_scope(Scope::Label("bug".into()), window, cx));
    pane.update(cx, |p, cx| p.choose(1, cx));
    settle(&pane, cx);
    assert_eq!(pane.read_with(cx, |p, _| p.scope().clone()), Scope::All);
}

#[gpui_kit::test]
fn signed_out_has_no_new_task_button(cx: &mut TestAppContext) {
    let provider = Fake::seeded("a", &["One"]);
    provider.fail(Operation::List, CapError::NotSignedIn);
    let (_pane, cx) = open_with(source_of(&[&provider]), 900., cx);
    assert!(cx.debug_bounds("tasks-new").is_none());
}

fn choices(pane: &Entity<TasksPane>, cx: &mut VisualTestContext) -> Vec<String> {
    pane.read_with(cx, |p, _| p.source.as_ref().map(|s| s.choices().iter().map(|c| c.words().to_string()).collect()).unwrap_or_default())
}

/// An account connected or forgotten in Settings reaches a pane that is open: the switcher follows, and the pane that was
/// showing the account goes back to the project's own tasks.
#[gpui_kit::test]
fn a_connected_account_joins_an_open_pane_and_leaves_with_its_provider(cx: &mut TestAppContext) {
    use crate::capability_hub::CapabilityHub;
    use atelier_capabilities::tasks::MemoryTasks;
    use gpui_kit::BorrowAppContext;
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
        cx.set_global(CapabilityHub::detached());
    });
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("atelier");
    std::fs::create_dir_all(&root).unwrap();
    let project: Arc<dyn atelier_project::Project> = Arc::new(atelier_project::LocalProject::open(&root).unwrap().with_data_dir(&base.path().join("data")));
    let (pane, cx) = cx.add_window_view(|window, cx| TasksPane::new("me", Vec::new(), window, cx));
    pane.update(cx, |p, cx| p.open_from(project, cx));
    settle(&pane, cx);
    assert_eq!(choices(&pane, cx), ["Local · atelier"], "one provider, no switcher");

    let hub = cx.update(|_, cx| cx.global::<CapabilityHub>().clone());
    let acme = Arc::new(MemoryTasks::new("acme"));
    let connect = |providers: Vec<Arc<dyn TasksProvider>>, cx: &mut VisualTestContext| {
        let turn = hub.checking(&Default::default());
        hub.install(crate::accounts::Built { rows: Default::default(), providers }, turn);
        cx.update(|_, cx| cx.update_global::<CapabilityHub, _>(|_, _| {}));
        settle(&pane, cx);
    };

    connect(vec![acme.clone()], cx);
    assert_eq!(choices(&pane, cx), ["Local · atelier", "Memory · acme"], "the switcher lists the account");
    assert!(cx.debug_bounds("tasks-provider").is_some(), "and is drawn");

    pane.update(cx, |p, cx| p.choose(1, cx));
    settle(&pane, cx);
    assert_eq!(pane.read_with(cx, |p, _| p.provider().map(|p| p.account().to_string())), Some("acme".into()));

    connect(Vec::new(), cx);
    assert_eq!(choices(&pane, cx), ["Local · atelier"]);
    assert_eq!(pane.read_with(cx, |p, _| p.provider().map(|p| p.account().to_string())), Some("atelier".into()), "back on the project's tasks");
}
