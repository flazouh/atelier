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
