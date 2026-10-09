use std::sync::Arc;

use atelier_capabilities::{
    Actor, Ref,
    tasks::{self as v1, Category, TasksProvider},
};
use atelier_tracker as tracker;
use atelier_tracker::{LocalTasks, Tracker as _};
use atelier_ui::{
    AgentLook,
    task_edit::Change,
    task_model::{Assignee, Label, Priority, TaskStatus},
};

use super::map::*;
use super::source::Vocabulary;

fn looks(_: &str) -> AgentLook {
    AgentLook::neutral(&atelier_ui::theme::Theme::light())
}

/// The local tracker as a provider, as the screen sees it.
fn local() -> (Arc<tracker::LocalTracker>, LocalTasks) {
    let tracker = Arc::new(tracker::LocalTracker::in_memory("LAT").unwrap());
    let provider = LocalTasks::new(tracker.clone(), "atelier", Actor::person("me", "me"));
    (tracker, provider)
}

fn vocab_of(provider: &dyn TasksProvider) -> Vocabulary {
    Vocabulary::new(provider.provider(), provider.account(), provider.capabilities(), provider.statuses().unwrap(), provider.labels().unwrap(), provider.projects().unwrap())
}

fn named(provider: &LocalTasks, title: &str, labels: &[&str]) -> v1::Task {
    let labels = labels.iter().map(|l| Ref::new("tasks", "local", "atelier", l).unwrap()).collect();
    provider.create(&v1::NewTask { title: title.into(), labels, ..v1::NewTask::default() }, &Actor::person("me", "me")).unwrap()
}

#[test]
fn a_task_reads_the_same_in_beui_as_in_the_provider() {
    let (tracker, provider) = local();
    let mut new = tracker::NewTask::titled("Fix the scroll");
    new.description = "It jumps.".into();
    new.priority = tracker::Priority::High;
    new.assignee = Some(tracker::Assignee::Agent("Claude".into()));
    new.labels = vec!["bug".into()];
    new.project = Some("Core".into());
    tracker.create(&new, "me").unwrap();
    let vocab = vocab_of(&provider);
    let made = provider.list(&v1::Query::default()).unwrap().items.remove(0);
    let lines = provider.activity(&made.reference, None).unwrap().items;
    let data = task_data(&made, &lines, &vocab, &looks);
    assert_eq!((data.key.as_ref(), data.title.as_ref()), ("LAT-1", "Fix the scroll"));
    assert_eq!(data.id.as_ref(), "tasks:local:atelier:LAT-1", "the row is named by the task's reference");
    assert_eq!(data.status, TaskStatus::Todo);
    assert_eq!(data.priority, Priority::High);
    assert_eq!(data.assignee.as_ref().map(|a| a.name().to_string()), Some("Claude".into()));
    assert!(matches!(data.assignee, Some(Assignee::Agent { .. })));
    assert_eq!(data.labels.iter().map(|l| l.name.to_string()).collect::<Vec<_>>(), ["bug"]);
    assert_eq!(data.project.as_deref(), Some("Core"));
    assert!(matches!(data.activity.first(), Some(atelier_ui::task_model::Activity::Created { .. })));
    assert!(data.created_at > 1_000_000_000 && data.created_at < 100_000_000_000, "seconds, not milliseconds: {}", data.created_at);
}

#[test]
fn a_parent_is_the_reference_of_its_row() {
    let (_, provider) = local();
    let parent = named(&provider, "Parent", &[]);
    let child = provider.create(&v1::NewTask { title: "Child".into(), parent: Some(parent.reference.clone()), ..v1::NewTask::default() }, &Actor::person("me", "me")).unwrap();
    let data = task_data(&child, &[], &vocab_of(&provider), &looks);
    assert_eq!(data.parent.as_deref(), Some(parent.reference.to_string().as_str()));
}

#[test]
fn the_sessions_and_pull_requests_of_a_local_task_still_show() {
    let (tracker, provider) = local();
    let made = named(&provider, "Linked", &[]);
    let id = tracker.list(&tracker::Query::default()).unwrap().remove(0).id;
    let link = tracker::SessionLink { session_id: "s1".into(), title: "Do it".into(), agent: "Claude".into() };
    tracker.record(&id, &tracker::Entry::SessionStarted(link), "Claude").unwrap();
    tracker.record(&id, &tracker::Entry::PrOpened(tracker::PrLink { number: 7, repo: "o/r".into() }), "me").unwrap();
    let again = provider.get(&made.reference).unwrap();
    let data = task_data(&again, &[], &vocab_of(&provider), &looks);
    assert_eq!(data.sessions.iter().map(|s| s.title.to_string()).collect::<Vec<_>>(), ["Do it"]);
    assert_eq!(data.prs.iter().map(|p| p.number).collect::<Vec<_>>(), [7]);
    let mut other = again.clone();
    other.raw = None;
    let bare = task_data(&other, &[], &vocab_of(&provider), &looks);
    assert!(bare.sessions.is_empty() && bare.prs.is_empty(), "a task of another provider has none here");
}

#[test]
fn the_activity_of_a_task_reads_as_lines() {
    use atelier_ui::task_model::Activity;
    let (tracker, provider) = local();
    let made = named(&provider, "Busy", &[]);
    let id = tracker.list(&tracker::Query::default()).unwrap().remove(0).id;
    tracker.record(&id, &tracker::Entry::Comment("Looks good".into()), "me").unwrap();
    let patch = v1::Patch { status: Some("done".into()), ..v1::Patch::default() };
    provider.update(&made.reference, &patch, &made.version, &Actor::person("me", "me")).unwrap();
    let lines = activity_data(&provider.activity(&made.reference, None).unwrap().items);
    assert!(matches!(lines[0], Activity::Created { .. }));
    assert!(lines.iter().any(|l| matches!(l, Activity::Comment { text, .. } if text.as_ref() == "Looks good")));
    assert!(lines.iter().any(|l| matches!(l, Activity::StatusChanged { from: TaskStatus::Todo, to: TaskStatus::Done, .. })));
}

#[test]
fn every_status_and_priority_maps_both_ways() {
    for status in [Category::Backlog, Category::Todo, Category::InProgress, Category::InReview, Category::Done, Category::Canceled] {
        assert_eq!(status_to(status_of(status)), status);
    }
    for priority in [v1::Priority::None, v1::Priority::Urgent, v1::Priority::High, v1::Priority::Medium, v1::Priority::Low] {
        assert_eq!(priority_to(priority_of(priority)), priority);
    }
}

#[test]
fn a_label_has_one_tone_on_every_run() {
    assert_eq!(label_of("bug"), label_of("bug"));
    assert!(label_of("bug").tone < 8);
}

#[test]
fn a_change_becomes_a_patch_for_each_task_it_names() {
    let (_, provider) = local();
    let (a, b) = (named(&provider, "A", &[]), named(&provider, "B", &[]));
    let vocab = vocab_of(&provider);
    let data: Vec<_> = [&a, &b].iter().map(|task| task_data(task, &[], &vocab, &looks)).collect();
    let ids = vec![data[0].id.clone()];
    let patches = patches_of(&data, &ids, &Change::Status(TaskStatus::Done), &vocab);
    assert_eq!(patches.len(), 1, "only the task named");
    assert_eq!(patches[0].0, a.reference);
    assert_eq!(patches[0].1.status.as_deref(), Some("done"));
}

#[test]
fn an_assignee_is_the_id_of_its_actor() {
    let (_, provider) = local();
    let a = named(&provider, "A", &[]);
    let vocab = vocab_of(&provider);
    let data = task_data(&a, &[], &vocab, &looks);
    let ids = vec![data.id.clone()];
    let agent = Assignee::agent("Claude", looks("Claude"));
    let patches = patches_of(std::slice::from_ref(&data), &ids, &Change::Assignee(Some(agent.clone())), &vocab);
    assert_eq!(patches[0].1.assignees, Some(vec!["agent:Claude".to_string()]));
    let cleared = patches_of(std::slice::from_ref(&data), &ids, &Change::Assignee(None), &vocab);
    assert_eq!(cleared[0].1.assignees, Some(vec![]), "an empty list clears");
    assert_eq!(actor_id_of(&Assignee::Person { name: "me".into() }), "me");
}

#[test]
fn a_label_toggles_off_only_when_every_task_has_it() {
    let (_, provider) = local();
    let (a, b) = (named(&provider, "A", &["bug"]), named(&provider, "B", &[]));
    let vocab = vocab_of(&provider);
    let data: Vec<_> = [&a, &b].iter().map(|task| task_data(task, &[], &vocab, &looks)).collect();
    let bug = Change::ToggleLabel(Label::new("bug", label_of("bug").tone));
    let bug_ref = Ref::new("tasks", "local", "atelier", "bug").unwrap();
    let both = vec![data[0].id.clone(), data[1].id.clone()];
    let patches = patches_of(&data, &both, &bug, &vocab);
    assert!(patches.iter().all(|(_, p)| p.labels == Some(vec![bug_ref.clone()])), "B lacks it, so it goes on both");
    let one = vec![data[0].id.clone()];
    let patches = patches_of(&data, &one, &bug, &vocab);
    assert_eq!(patches[0].1.labels, Some(vec![]), "A alone has it, so it comes off");
}

#[test]
fn a_new_task_carries_the_draft_and_a_status_the_provider_lacks_is_refused() {
    let (_, provider) = local();
    named(&provider, "Seen", &["bug"]);
    let vocab = vocab_of(&provider);
    let mut draft = atelier_ui::new_task_model::Draft { title: "  Make it  ".into(), ..Default::default() };
    draft.labels = vec![Label::new("bug", 0)];
    draft.assignee = Some(Assignee::agent("Claude", looks("Claude")));
    let new = new_task_of(&draft, &vocab).unwrap();
    assert_eq!(new.title, "Make it");
    assert_eq!(new.labels, vec![Ref::new("tasks", "local", "atelier", "bug").unwrap()]);
    assert_eq!(new.assignees, ["agent:Claude"]);
    let few = Vocabulary::new("local", "atelier", provider.capabilities(), vec![v1::Status::plain(Category::Todo)], vec![], vec![]);
    draft.status = TaskStatus::Done;
    assert!(new_task_of(&draft, &few).is_err(), "this provider has no Done");
}

#[test]
fn the_first_message_names_the_task_and_carries_its_words() {
    let (_, provider) = local();
    let described = provider.create(&v1::NewTask { title: "Add a line to the README".into(), description: "Say hello.".into(), ..v1::NewTask::default() }, &Actor::person("me", "me")).unwrap();
    assert_eq!(first_message(&described), "LAT-1: Add a line to the README\n\nSay hello.\n\nWork on this task. The task is LAT-1.");
    let bare = named(&provider, "Bare", &[]);
    assert_eq!(first_message(&bare), "LAT-2: Bare\n\nWork on this task. The task is LAT-2.");
}

#[test]
fn a_key_finds_the_local_id_for_the_rules() {
    let (tracker, provider) = local();
    named(&provider, "One", &[]);
    let id = tracker.list(&tracker::Query::default()).unwrap().remove(0).id;
    assert_eq!(local_id(tracker.as_ref(), "LAT-1"), Some(id));
    assert_eq!(local_id(tracker.as_ref(), "LAT-9"), None);
}

#[test]
fn a_local_projects_tasks_stay_in_its_data_folder_across_opens() {
    use atelier_project::{LocalProject, Project};
    let root = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project = LocalProject::open(root.path()).unwrap().with_data_dir(data.path());
    let first = project.tracker().expect("a local project has a tracker");
    first.create(&tracker::NewTask::titled("Keep me"), "me").unwrap();
    drop(first);
    let again = LocalProject::open(root.path()).unwrap().with_data_dir(data.path()).tracker().unwrap();
    let tasks = again.list(&tracker::Query::default()).unwrap();
    assert_eq!(tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Keep me"]);
    assert!(!root.path().join(atelier_project::TRACKER_FILE).exists(), "nothing in the repository");
}

#[test]
fn a_session_tells_its_task_what_happened() {
    use super::signal::{SessionRef, TaskEvent, of};
    let task = super::TaskRef { id: tracker::TaskId::from("1"), key: "LAT-1".into() };
    let session = SessionRef { id: "s1", title: "Do it", agent: "Claude" };
    assert!(matches!(of(TaskEvent::Started, Some(&task), &session), Some(tracker::Signal::SessionStarted { .. })));
    assert_eq!(of(TaskEvent::Started, None, &session), None, "a session that began elsewhere links nothing");
    assert_eq!(of(TaskEvent::TurnEnded { ok: false }, None, &session), Some(tracker::Signal::SessionFinished { session_id: "s1".into(), ok: false }));
    assert_eq!(of(TaskEvent::Replied, None, &session), Some(tracker::Signal::SessionResumed { session_id: "s1".into() }));
}

#[test]
fn a_commit_and_a_pull_request_reach_the_tasks_of_the_session() {
    use super::signal::{SessionRef, TaskEvent, of};
    let task = super::TaskRef { id: tracker::TaskId::from("1"), key: "LAT-1".into() };
    let session = SessionRef { id: "s1", title: "Do it", agent: "Claude" };
    let commit = TaskEvent::Committed { sha: "abc1234def".into(), subject: "Fix it".into() };
    assert_eq!(
        of(commit, None, &session),
        Some(tracker::Signal::Committed { session_id: "s1".into(), sha: "abc1234def".into(), subject: "Fix it".into(), by: "you".into() })
    );
    let opened = TaskEvent::PrOpened { number: 9, repo: "o/r".into() };
    assert_eq!(
        of(opened.clone(), Some(&task), &session),
        Some(tracker::Signal::PrOpened { task: tracker::TaskId::from("1"), pr: tracker::PrLink { number: 9, repo: "o/r".into() }, by: "you".into() })
    );
    assert_eq!(of(opened, None, &session), None, "a session with no task opens no link");
}
