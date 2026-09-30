use beui::{
    AgentLook,
    task_edit::Change,
    task_model::{Assignee, Label, Priority, TaskStatus},
};
use lathe_tracker as tracker;
use lathe_tracker::Tracker as _;

use super::map::*;

fn looks(_: &str) -> AgentLook {
    AgentLook::neutral(&beui::theme::Theme::light())
}

fn tracker() -> tracker::LocalTracker {
    tracker::LocalTracker::in_memory("LAT").unwrap()
}

#[test]
fn a_task_reads_the_same_in_beui_as_in_the_tracker() {
    let t = tracker();
    let mut new = tracker::NewTask::titled("Fix the scroll");
    new.description = "It jumps.".into();
    new.priority = tracker::Priority::High;
    new.assignee = Some(tracker::Assignee::Agent("Claude".into()));
    new.labels = vec!["bug".into()];
    let made = t.create(&new, "me").unwrap();
    let data = task_data(&made, &t.activity(&made.id).unwrap(), &looks);
    assert_eq!((data.key.as_ref(), data.title.as_ref()), ("LAT-1", "Fix the scroll"));
    assert_eq!(data.status, TaskStatus::Todo);
    assert_eq!(data.priority, Priority::High);
    assert_eq!(data.assignee.as_ref().map(|a| a.name().to_string()), Some("Claude".into()));
    assert!(matches!(data.assignee, Some(Assignee::Agent { .. })));
    assert_eq!(data.labels.iter().map(|l| l.name.to_string()).collect::<Vec<_>>(), ["bug"]);
    assert!(matches!(data.activity.first(), Some(beui::task_model::Activity::Created { .. })));
}

#[test]
fn every_status_and_priority_maps_both_ways() {
    for status in tracker::Status::ALL {
        assert_eq!(status_to(status_of(status)), status);
    }
    for priority in tracker::Priority::ALL {
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
    let t = tracker();
    let a = t.create(&tracker::NewTask::titled("A"), "me").unwrap();
    let b = t.create(&tracker::NewTask::titled("B"), "me").unwrap();
    let data: Vec<_> = [&a, &b].iter().map(|task| task_data(task, &[], &looks)).collect();
    let ids = vec![data[0].id.clone()];
    let patches = patches_of(&data, &ids, &Change::Status(TaskStatus::Done));
    assert_eq!(patches.len(), 1, "only the task named");
    assert_eq!(patches[0].0, a.id);
    assert_eq!(patches[0].1.status, Some(tracker::Status::Done));
}

#[test]
fn a_label_toggles_off_only_when_every_task_has_it() {
    let t = tracker();
    let mut new = tracker::NewTask::titled("A");
    new.labels = vec!["bug".into()];
    let a = t.create(&new, "me").unwrap();
    let b = t.create(&tracker::NewTask::titled("B"), "me").unwrap();
    let data: Vec<_> = [&a, &b].iter().map(|task| task_data(task, &[], &looks)).collect();
    let bug = Change::ToggleLabel(Label::new("bug", label_of("bug").tone));
    let both = vec![data[0].id.clone(), data[1].id.clone()];
    let patches = patches_of(&data, &both, &bug);
    assert!(patches.iter().all(|(_, p)| p.add_labels == ["bug"] && p.remove_labels.is_empty()), "B lacks it, so it goes on both");
    let one = vec![data[0].id.clone()];
    let patches = patches_of(&data, &one, &bug);
    assert_eq!(patches[0].1.remove_labels, ["bug"], "A alone has it, so it comes off");
}

#[test]
fn the_first_message_names_the_task_and_carries_its_words() {
    let t = tracker();
    let mut new = tracker::NewTask::titled("Add a line to the README");
    new.description = "Say hello.".into();
    let task = t.create(&new, "me").unwrap();
    assert_eq!(first_message(&task), "LAT-1: Add a line to the README\n\nSay hello.\n\nWork on this task. The task is LAT-1.");
    let bare = t.create(&tracker::NewTask::titled("Bare"), "me").unwrap();
    assert_eq!(first_message(&bare), "LAT-2: Bare\n\nWork on this task. The task is LAT-2.");
}

#[test]
fn a_local_projects_tasks_stay_in_its_data_folder_across_opens() {
    use lathe_project::LocalProject;
    let root = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project = LocalProject::open(root.path()).unwrap().with_data_dir(data.path());
    let location = lathe_settings::Location::Local { path: root.path().to_path_buf() };
    let first = super::store::open(&location, &project, "lathe").expect("a local project has a tracker");
    first.create(&tracker::NewTask::titled("Keep me"), "me").unwrap();
    drop(first);
    let again = super::store::open(&location, &project, "lathe").unwrap();
    let titles: Vec<_> = again.list(&tracker::Query::default()).unwrap().into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["Keep me"]);
    assert!(!root.path().join(super::store::FILE).exists(), "nothing in the repository");
    let key = again.list(&tracker::Query::default()).unwrap()[0].key.clone();
    assert!(key.starts_with("LAT-"), "the prefix comes from the project name: {key}");
}

#[test]
fn a_project_over_ssh_says_its_tasks_come_later() {
    use lathe_project::LocalProject;
    let root = tempfile::tempdir().unwrap();
    let project = LocalProject::open(root.path()).unwrap();
    let location = lathe_settings::Location::Ssh { host: "hp".into(), path: "/srv/x".into() };
    let why = super::store::open(&location, &project, "x").err().expect("no tracker yet");
    assert!(why.contains("SSH"), "{why}");
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
