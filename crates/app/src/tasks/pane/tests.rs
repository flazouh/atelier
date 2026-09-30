use std::sync::Arc;

use beui::task_model::TaskStatus;
use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext, px, size};
use lathe_tracker::{LocalTracker, NewTask, Query, Tracker};

use super::*;

fn open(width: f32, cx: &mut TestAppContext) -> (Entity<TasksPane>, Arc<LocalTracker>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let tracker = Arc::new(LocalTracker::in_memory("LAT").unwrap());
    tracker.create(&NewTask::titled("Fix the scroll"), "me").unwrap();
    tracker.create(&NewTask::titled("Write the docs"), "me").unwrap();
    let handle: Arc<dyn Tracker> = tracker.clone();
    let (pane, cx) = cx.add_window_view(move |window, cx| {
        TasksPane::new(Ok(handle), "me", vec![("Claude".into(), lathe_agents::claude::look())], window, cx)
    });
    cx.simulate_resize(size(px(width), px(700.)));
    settle(&pane, cx);
    (pane, tracker, cx)
}

fn settle(pane: &Entity<TasksPane>, cx: &mut VisualTestContext) {
    for _ in 0..6 {
        cx.run_until_parked();
        pane.update(cx, |_, cx| cx.notify());
    }
}

#[gpui_kit::test]
fn the_pane_reads_the_projects_tasks_off_the_ui_thread(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    let titles = pane.read_with(cx, |p, _| p.tasks().iter().map(|t| t.title.to_string()).collect::<Vec<_>>());
    assert_eq!(titles.len(), 2);
    assert!(titles.contains(&"Fix the scroll".to_string()));
    assert!(cx.debug_bounds("tasks-mode-list").is_some(), "the List | Board switch is drawn");
}

#[gpui_kit::test]
fn a_change_in_the_pane_reaches_the_tracker(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    let id = pane.read_with(cx, |p, _| p.tasks()[0].id.clone());
    pane.update(cx, |p, cx| p.changed(std::slice::from_ref(&id), &Change::Status(TaskStatus::Done), Source::None, cx));
    settle(&pane, cx);
    let saved = tracker.get(&TaskId(id.to_string())).unwrap().unwrap();
    assert_eq!(saved.status, lathe_tracker::Status::Done);
    let line = tracker.activity(&saved.id).unwrap();
    assert!(line.iter().any(|a| a.by == "me"), "the change is logged as the reader's");
}

#[gpui_kit::test]
fn a_narrow_pane_shows_the_list_and_hides_the_switch(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(450., cx);
    pane.update(cx, |p, cx| {
        p.mode = Mode::Board;
        cx.notify();
    });
    settle(&pane, cx);
    assert_eq!(pane.read_with(cx, |p, _| p.shown_mode()), Mode::List, "the board would clip under {BOARD_LEAST} px");
    assert!(cx.debug_bounds("tasks-mode-list").is_none(), "no switch when there is one choice");
}

#[gpui_kit::test]
fn a_wide_pane_can_show_the_board(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    pane.update(cx, |p, cx| {
        p.mode = Mode::Board;
        cx.notify();
    });
    settle(&pane, cx);
    assert_eq!(pane.read_with(cx, |p, _| p.shown_mode()), Mode::Board);
}

#[gpui_kit::test]
fn a_project_with_no_tracker_says_why(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
    });
    let (pane, cx) = cx.add_window_view(|window, cx| TasksPane::new(Err("Tasks for a remote project come with the next helper.".into()), "me", Vec::new(), window, cx));
    cx.simulate_resize(size(px(600.), px(400.)));
    settle(&pane, cx);
    assert!(pane.read_with(cx, |p, _| matches!(p.load, Load::Failed(_))));
    let _ = Query::default();
}
