use std::sync::Arc;

use beui::task_model::TaskStatus;
use gpui_kit::{Entity, TestAppContext, VisualTestContext, px, size};
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
        let mut pane = TasksPane::new("me", vec![("Claude".into(), lathe_agents::claude::look())], window, cx);
        pane.attach(Ok(handle), cx);
        pane
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
    let (pane, cx) = cx.add_window_view(|window, cx| {
        let mut pane = TasksPane::new("me", Vec::new(), window, cx);
        pane.attach(Err("Tasks for a remote project come with the next helper.".into()), cx);
        pane
    });
    cx.simulate_resize(size(px(600.), px(400.)));
    settle(&pane, cx);
    assert!(pane.read_with(cx, |p, _| matches!(p.load, Load::Failed(_))));
    let _ = Query::default();
}

/// The keys of the list work as soon as the pane has the focus: the pane hands the focus to its list.
#[gpui_kit::test]
fn the_pane_hands_the_focus_to_the_part_that_reads_the_keys(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    let handle = cx.update(|_, cx| pane.focus_handle(cx));
    cx.update(|window, cx| handle.focus(window, cx));
    settle(&pane, cx);
    let list = pane.read_with(cx, |p, _| p.list.clone());
    assert!(cx.update(|window, cx| list.focus_handle(cx).is_focused(window)), "the list holds the focus");
}

/// A new task takes what the reader types at once: the caret is in the title when the dialog opens.
#[gpui_kit::test]
fn the_new_task_dialog_puts_the_caret_in_the_title(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    settle(&pane, cx);
    cx.simulate_input("Ship it");
    settle(&pane, cx);
    let title = pane.read_with(cx, |p, cx| p.dialog.read(cx).draft().title.clone());
    assert_eq!(title, "Ship it");
}

/// A press on a row of a picker chooses it, in the list, the board, the task in full and the dialog. These press the
/// rows with real mouse events.
fn press_row(cx: &mut VisualTestContext, row: usize) {
    const ROWS: [&str; 6] = ["picker-row-0", "picker-row-1", "picker-row-2", "picker-row-3", "picker-row-4", "picker-row-5"];
    let at = cx.debug_bounds(ROWS[row]).unwrap_or_else(|| panic!("row {row} is drawn")).center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    cx.run_until_parked();
}

fn focus_body(pane: &Entity<TasksPane>, cx: &mut VisualTestContext) {
    let handle = cx.update(|_, cx| pane.focus_handle(cx));
    cx.update(|window, cx| handle.focus(window, cx));
    settle(pane, cx);
}

fn first_status(tracker: &LocalTracker) -> lathe_tracker::Status {
    tracker.list(&Query::default()).unwrap().into_iter().find(|t| t.title == "Fix the scroll").unwrap().status
}

#[gpui_kit::test]
fn a_press_on_a_picker_row_chooses_it_in_the_list(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    focus_body(&pane, cx);
    // The first row is the group heading; the second is a task. `s` opens its status picker; Done is the fifth row.
    cx.simulate_keystrokes("j j s");
    settle(&pane, cx);
    press_row(cx, 4);
    settle(&pane, cx);
    let statuses: Vec<_> = tracker.list(&Query::default()).unwrap().into_iter().map(|t| t.status).collect();
    assert!(statuses.contains(&lathe_tracker::Status::Done), "one task is Done now: {statuses:?}");
    assert!(cx.debug_bounds("picker-row-0").is_none(), "the picker closed");
}

#[gpui_kit::test]
fn a_press_on_a_picker_row_chooses_it_in_the_board(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    pane.update(cx, |p, cx| {
        p.mode = Mode::Board;
        cx.notify();
    });
    settle(&pane, cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("j s");
    settle(&pane, cx);
    press_row(cx, 4);
    settle(&pane, cx);
    let statuses: Vec<_> = tracker.list(&Query::default()).unwrap().into_iter().map(|t| t.status).collect();
    assert!(statuses.contains(&lathe_tracker::Status::Done), "{statuses:?}");
}

#[gpui_kit::test]
fn a_press_on_a_picker_row_chooses_it_in_the_task_in_full(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    let id = pane.read_with(cx, |p, _| p.tasks().iter().find(|t| t.title == "Fix the scroll").unwrap().id.clone());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.show(id, window, cx)));
    settle(&pane, cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("s");
    settle(&pane, cx);
    press_row(cx, 4);
    settle(&pane, cx);
    assert_eq!(first_status(&tracker), lathe_tracker::Status::Done);
}

#[gpui_kit::test]
fn a_press_in_the_dialog_chooses_an_assignee_and_a_press_on_the_description_puts_the_caret_there(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    settle(&pane, cx);
    let button = cx.debug_bounds("new-task-assignee").expect("the assignee button is drawn").center();
    cx.simulate_click(button, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    press_row(cx, 2);
    settle(&pane, cx);
    let who = pane.read_with(cx, |p, cx| p.dialog.read(cx).draft().assignee.as_ref().map(|a| a.name().to_string()));
    assert_eq!(who.as_deref(), Some("Claude"), "Unassigned, me, Claude: the third row");
    let description = cx.debug_bounds("new-task-description").expect("the description is drawn").center();
    cx.simulate_click(description, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    cx.simulate_input("Say hi");
    settle(&pane, cx);
    assert_eq!(pane.read_with(cx, |p, cx| p.dialog.read(cx).description_text(cx)), "Say hi");
}

fn types_in_the_dialog(pane: &Entity<TasksPane>, cx: &mut VisualTestContext, title: &str) {
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    settle(pane, cx);
    cx.simulate_input(title);
    settle(pane, cx);
}

/// The create key works from the description too, not only from the title.
#[gpui_kit::test]
fn command_enter_creates_from_the_description_too(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    types_in_the_dialog(&pane, cx, "Ship it");
    let description = cx.debug_bounds("new-task-description").expect("the description is drawn").center();
    cx.simulate_click(description, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    cx.simulate_input("with a description");
    settle(&pane, cx);
    cx.simulate_keystrokes("ctrl-enter");
    settle(&pane, cx);
    let made = tracker.list(&Query::default()).unwrap().into_iter().find(|t| t.title == "Ship it").expect("the task was made");
    assert_eq!(made.description, "with a description");
    assert!(!pane.read_with(cx, |p, _| p.creating), "the dialog closed");
}

/// Escape on a dialog with typed text asks first, and a second Escape keeps the draft.
#[gpui_kit::test]
fn escape_on_a_typed_dialog_asks_before_it_drops_the_draft(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    types_in_the_dialog(&pane, cx, "A draft");
    cx.simulate_keystrokes("escape");
    settle(&pane, cx);
    assert!(pane.read_with(cx, |p, _| p.creating), "the dialog is still open");
    assert!(cx.debug_bounds("discard-question").is_some(), "it asks");
    cx.simulate_keystrokes("escape");
    settle(&pane, cx);
    assert!(pane.read_with(cx, |p, _| p.creating), "Escape again keeps editing");
    assert!(cx.debug_bounds("discard-question").is_none(), "the question is gone");
    assert_eq!(pane.read_with(cx, |p, cx| p.dialog.read(cx).draft().title.clone()), "A draft", "the draft is kept");
    cx.simulate_keystrokes("escape");
    settle(&pane, cx);
    let discard = cx.debug_bounds("discard-new-task").expect("the discard button is drawn").center();
    cx.simulate_click(discard, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    assert!(!pane.read_with(cx, |p, _| p.creating), "Discard closes it");
}

#[gpui_kit::test]
fn escape_on_an_empty_dialog_closes_it_at_once(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    settle(&pane, cx);
    cx.simulate_keystrokes("escape");
    settle(&pane, cx);
    assert!(!pane.read_with(cx, |p, _| p.creating));
}

/// The list has a cursor on a task when it opens, and on the new task after Create, so `s` works at once.
#[gpui_kit::test]
fn the_list_has_a_cursor_when_it_opens_so_a_key_works_at_once(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("s");
    settle(&pane, cx);
    assert!(cx.debug_bounds("picker-row-0").is_some(), "the status picker opened with no move first");
}

#[gpui_kit::test]
fn after_create_the_cursor_is_on_the_new_task(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    let draft = beui::new_task_model::Draft { title: "Brand new".into(), ..Default::default() };
    cx.update(|window, cx| pane.update(cx, |p, cx| p.create(&draft, false, window, cx)));
    settle(&pane, cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("s");
    settle(&pane, cx);
    press_row(cx, 4);
    settle(&pane, cx);
    let new = tracker.list(&Query::default()).unwrap().into_iter().find(|t| t.title == "Brand new").unwrap();
    assert_eq!(new.status, lathe_tracker::Status::Done, "the key acted on the new task");
}

/// The board has the same default cursor: on the first card when it opens, and on the new card after Create.
#[gpui_kit::test]
fn the_board_has_a_cursor_when_it_opens_so_a_key_works_at_once(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    pane.update(cx, |p, cx| {
        p.mode = Mode::Board;
        cx.notify();
    });
    settle(&pane, cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("s");
    settle(&pane, cx);
    assert!(cx.debug_bounds("picker-row-0").is_some(), "the status picker opened with no move first");
}

#[gpui_kit::test]
fn after_create_the_board_cursor_is_on_the_new_card(cx: &mut TestAppContext) {
    let (pane, tracker, cx) = open(900., cx);
    pane.update(cx, |p, cx| {
        p.mode = Mode::Board;
        cx.notify();
    });
    settle(&pane, cx);
    let draft = beui::new_task_model::Draft { title: "Brand new card".into(), ..Default::default() };
    cx.update(|window, cx| pane.update(cx, |p, cx| p.create(&draft, false, window, cx)));
    settle(&pane, cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("s");
    settle(&pane, cx);
    press_row(cx, 4);
    settle(&pane, cx);
    let new = tracker.list(&Query::default()).unwrap().into_iter().find(|t| t.title == "Brand new card").unwrap();
    assert_eq!(new.status, lathe_tracker::Status::Done, "the key acted on the new card");
}

fn picker_rows(cx: &mut VisualTestContext) -> usize {
    const ROWS: [&str; 8] = ["picker-row-0", "picker-row-1", "picker-row-2", "picker-row-3", "picker-row-4", "picker-row-5", "picker-row-6", "picker-row-7"];
    ROWS.iter().take_while(|name| cx.debug_bounds(name).is_some()).count()
}

/// Every picker of the new task dialog grows out of its field's chip: the surface starts on the chip's corner, its
/// header row is the filter, and its rows sit under the header.
#[gpui_kit::test]
fn each_picker_of_the_dialog_grows_out_of_its_chip(cx: &mut TestAppContext) {
    for (chip, field) in [("new-task-status", "Status"), ("new-task-priority", "Priority"), ("new-task-assignee", "Assignee"), ("new-task-labels", "Labels")] {
        let (pane, _, cx) = open(900., cx);
        cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
        settle(&pane, cx);
        let button = cx.debug_bounds(chip).unwrap_or_else(|| panic!("{chip} is drawn"));
        cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
        settle(&pane, cx);
        let surface = cx.debug_bounds("picker-surface").unwrap_or_else(|| panic!("{field}: the surface is drawn"));
        assert!((f32::from(surface.left() - button.left())).abs() <= 1. && (f32::from(surface.top() - button.top())).abs() <= 1., "{field}: it starts on its chip: {surface:?} from {button:?}");
        let header = cx.debug_bounds("picker-header").expect("the header is drawn");
        assert_eq!(header.size.height, button.size.height, "{field}: the header is the chip's height");
        assert!(cx.debug_bounds("picker-filter").is_some(), "{field}: the header is the filter");
        // The test tracker has no labels, so that picker shows its empty words and no rows.
        if field == "Labels" {
            assert!(surface.size.height > header.size.height, "{field}: it grew");
            continue;
        }
        let first = cx.debug_bounds("picker-row-0").unwrap_or_else(|| panic!("{field}: the rows are drawn"));
        assert!(first.top() >= header.bottom(), "{field}: the rows are under the header");
        assert!(surface.size.height > header.size.height, "{field}: it grew");
    }
}

/// Typing filters the open picker, in the header row, and Enter chooses the first row left. A label picker stays open.
#[gpui_kit::test]
fn typing_filters_each_picker_and_enter_chooses(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(900., cx);
    cx.update(|window, cx| pane.update(cx, |p, cx| p.new_task(window, cx)));
    settle(&pane, cx);
    let assignee = cx.debug_bounds("new-task-assignee").unwrap().center();
    cx.simulate_click(assignee, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    let before = picker_rows(cx);
    cx.simulate_keystrokes("c l");
    settle(&pane, cx);
    let after = picker_rows(cx);
    assert!(after >= 1 && after < before, "typing narrowed {before} rows to {after}");
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    let who = pane.read_with(cx, |p, cx| p.dialog.read(cx).draft().assignee.as_ref().map(|a| a.name().to_string()));
    assert_eq!(who.as_deref(), Some("Claude"), "Enter chose the row left");
    assert!(cx.debug_bounds("picker-surface").is_none(), "the surface went back to its chip");

    let status = cx.debug_bounds("new-task-status").unwrap().center();
    cx.simulate_click(status, gpui_kit::Modifiers::default());
    settle(&pane, cx);
    let before = picker_rows(cx);
    cx.simulate_keystrokes("d o n e");
    settle(&pane, cx);
    assert!(picker_rows(cx) < before);
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    let status = pane.read_with(cx, |p, cx| p.dialog.read(cx).draft().status);
    assert_eq!(status.words(), "Done");
}

/// A filter chip of the list opens its picker as a surface grown out of the chip; typing filters it and Enter chooses.
#[gpui_kit::test]
fn a_filter_chip_grows_into_its_picker_and_typing_filters(cx: &mut TestAppContext) {
    for chip in ["filter-assignee", "filter-label", "filter-priority"] {
        let (pane, _, cx) = open(900., cx);
        focus_body(&pane, cx);
        let button = cx.debug_bounds(chip).unwrap_or_else(|| panic!("{chip} is drawn"));
        cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
        settle(&pane, cx);
        let surface = cx.debug_bounds("picker-surface").unwrap_or_else(|| panic!("{chip}: the surface is drawn"));
        assert!((f32::from(surface.left() - button.left())).abs() <= 1. && (f32::from(surface.top() - button.top())).abs() <= 1., "{chip}: it starts on its chip: {surface:?} from {button:?}");
        assert!(cx.debug_bounds("picker-filter").is_some(), "{chip}: the header is the filter");
        assert!(surface.size.height > button.size.height, "{chip}: it grew");
        assert_eq!(cx.debug_bounds("picker-header").unwrap().size.height, button.size.height);
    }
    let (pane, _, cx) = open(900., cx);
    focus_body(&pane, cx);
    let button = cx.debug_bounds("filter-priority").unwrap();
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    settle(&pane, cx);
    let before = picker_rows(cx);
    cx.simulate_keystrokes("u r");
    settle(&pane, cx);
    let after = picker_rows(cx);
    assert!(after >= 1 && after < before, "typing narrowed {before} rows to {after}");
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    assert!(cx.debug_bounds("picker-surface").is_none(), "the surface went back to its chip");
}

/// A property of the task in full opens its picker as a surface grown out of its button, and typing filters it.
#[gpui_kit::test]
fn a_property_of_the_task_grows_into_its_picker_and_typing_filters(cx: &mut TestAppContext) {
    let (pane, _, cx) = open(1100., cx);
    focus_body(&pane, cx);
    cx.simulate_keystrokes("j j enter");
    settle(&pane, cx);
    for (chip, field) in [("rail-status", "Status"), ("rail-priority", "Priority"), ("rail-assignee", "Assignee"), ("rail-labels", "Labels")] {
        let button = cx.debug_bounds(chip).unwrap_or_else(|| panic!("{chip} is drawn"));
        cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
        settle(&pane, cx);
        let surface = cx.debug_bounds("picker-surface").unwrap_or_else(|| panic!("{field}: the surface is drawn"));
        let right = (f32::from(surface.right() - button.right())).abs() <= 1.;
        let left = (f32::from(surface.left() - button.left())).abs() <= 1.;
        assert!(left || right, "{field}: it starts on its button's corner: {surface:?} from {button:?}");
        assert!((f32::from(surface.top() - button.top())).abs() <= 1., "{field}: and its top");
        assert!(cx.debug_bounds("picker-filter").is_some(), "{field}: the header is the filter");
        cx.simulate_keystrokes("escape");
        settle(&pane, cx);
        assert!(cx.debug_bounds("picker-surface").is_none(), "{field}: Escape sent it back");
    }
    let button = cx.debug_bounds("rail-status").unwrap();
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    settle(&pane, cx);
    let before = picker_rows(cx);
    cx.simulate_keystrokes("d o");
    settle(&pane, cx);
    assert!(picker_rows(cx) < before, "typing filtered the status picker");
}
