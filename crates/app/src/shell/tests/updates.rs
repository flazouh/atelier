use std::rc::Rc;

use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::{open_shell, settle, with_a_session};
use crate::{
    shell::{CheckForUpdates, Shell},
    updater::{
        NoDriver, RequestSender, Requests, UpdateEvent, UpdateState, Updater,
        fakes::{Calls, Counting, waiting},
    },
};

fn ask_for_updates(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update_in(cx, |s, window, cx| s.check_for_updates(&CheckForUpdates, window, cx));
    settle(shell, cx);
}

/// Makes the one open tab hold an edit that is not saved.
fn leave_an_edit_unsaved(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update_in(cx, |s, window, cx| {
        s.active().unwrap().clone().update(cx, |p, cx| p.open_file("a.txt", window, cx));
    });
    settle(shell, cx);
    shell.update_in(cx, |s, _, cx| {
        s.active().unwrap().clone().update(cx, |p, _| p.buffers.get_mut("a.txt").unwrap().dirty = true);
    });
    assert_eq!(shell.read_with(cx, |s, cx| s.unsaved(cx)), 1);
}

fn requests() -> (RequestSender, Requests) {
    futures_channel::mpsc::unbounded()
}

#[gpui_kit::test]
fn asking_for_updates_in_a_build_that_cannot_update_says_so_in_a_notice(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    cx.update(|_, cx| cx.set_global(Updater::new(Rc::new(NoDriver))));
    ask_for_updates(&shell, cx);
    assert!(cx.debug_bounds("notice").is_some(), "the notice shows");
}

#[gpui_kit::test]
fn asking_for_updates_hands_the_check_to_the_updater_and_says_it_looks(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (driver, calls) = Counting::recording(true);
    cx.update(|_, cx| cx.set_global(Updater::new(Rc::new(driver))));
    ask_for_updates(&shell, cx);
    assert_eq!(calls.counts(), (1, 1, 0, 0), "one look, and the reader asked for it");
    assert!(cx.debug_bounds("notice").is_some(), "a notice says it looks, since the updater has no window of its own");
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Checking { asked: true });
}

/// A shell with a recording updater.
fn with_an_updater(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Calls, tempfile::TempDir) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    let (driver, calls) = Counting::recording(true);
    cx.update(|_, cx| cx.set_global(Updater::new(Rc::new(driver))));
    (shell, cx, calls, dir)
}

fn tell(shell: &Entity<Shell>, cx: &mut VisualTestContext, event: UpdateEvent) {
    shell.update(cx, |s, cx| s.update_event(event, cx));
    settle(shell, cx);
}

/// The daily look finds an update: it downloads with a quiet percentage in the title bar, and when it is ready a button waits.
fn found_by_the_daily_look(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    tell(shell, cx, UpdateEvent::Checking { user: false });
    tell(shell, cx, UpdateEvent::Found { version: "0.2.0".into(), notes: Some("## What is new\n\n- A thing".into()), user: false });
}

#[gpui_kit::test]
fn an_update_the_daily_look_finds_downloads_in_silence_and_then_waits_behind_a_button(cx: &mut TestAppContext) {
    let (shell, cx, _calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Downloading { fraction: 0.5 });
    assert!(cx.debug_bounds("update-progress").is_some(), "the download shows a percentage");
    assert!(cx.debug_bounds("update-chip").is_none() && cx.debug_bounds("release-sheet").is_none() && cx.debug_bounds("notice").is_none(), "and nothing else");
    tell(&shell, cx, UpdateEvent::Extracting { fraction: 1. });
    tell(&shell, cx, UpdateEvent::Ready);
    assert!(cx.debug_bounds("update-chip").is_some(), "a button says the update is ready");
    assert!(cx.debug_bounds("update-progress").is_none() && cx.debug_bounds("release-sheet").is_none(), "the changelog does not open by itself");
}

#[gpui_kit::test]
fn the_button_opens_the_changelog_and_restart_installs_the_update(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    let chip = cx.debug_bounds("update-chip").expect("the button is there");
    cx.simulate_click(chip.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some(), "the changelog opens");
    let restart = cx.debug_bounds("release-install").expect("with a button to restart");
    cx.simulate_click(restart.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(calls.counts(), (0, 0, 1, 0), "the updater was told to install");
    assert!(cx.debug_bounds("release-sheet").is_none(), "the panel is gone");
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Installing);
}

#[gpui_kit::test]
fn later_keeps_the_update_for_the_next_quit_and_so_does_escape(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    shell.update(cx, |s, cx| s.show_update(cx));
    settle(&shell, cx);
    let later = cx.debug_bounds("release-later").expect("Later is there");
    cx.simulate_click(later.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(calls.counts(), (0, 0, 0, 1));
    assert!(cx.debug_bounds("release-sheet").is_none() && cx.debug_bounds("notice").is_some(), "the panel is gone and a notice says when it installs");
    assert!(cx.debug_bounds("update-chip").is_none(), "nothing is left to press: the updater installs it when the app quits");
    // Escape is Later too.
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    shell.update(cx, |s, cx| s.show_update(cx));
    settle(&shell, cx);
    cx.simulate_keystrokes("escape");
    settle(&shell, cx);
    assert_eq!(calls.counts(), (0, 0, 0, 2));
}

#[gpui_kit::test]
fn a_look_the_reader_asked_for_opens_the_changelog_by_itself_and_says_when_nothing_is_newer(cx: &mut TestAppContext) {
    let (shell, cx, _calls, _dir) = with_an_updater(cx);
    ask_for_updates(&shell, cx);
    tell(&shell, cx, UpdateEvent::UpToDate);
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Idle);
    ask_for_updates(&shell, cx);
    tell(&shell, cx, UpdateEvent::Found { version: "0.2.0".into(), notes: None, user: true });
    tell(&shell, cx, UpdateEvent::Ready);
    assert!(cx.debug_bounds("release-sheet").is_some(), "the reader asked, so the update opens by itself");
}

#[gpui_kit::test]
fn asking_again_while_an_update_is_ready_opens_it_and_does_not_look_again(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    ask_for_updates(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some());
    assert_eq!(calls.counts().0, 0, "no second look");
}

#[gpui_kit::test]
fn asking_for_updates_before_an_updater_is_set_up_says_so_on_the_start_screen(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    ask_for_updates(&shell, cx);
    assert!(cx.debug_bounds("start-error").is_some(), "the start screen says the build cannot update");
}

#[gpui_kit::test]
fn an_update_restarts_a_window_with_nothing_unsaved_without_asking(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (request, answers) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    assert!(!cx.has_pending_prompt(), "a clean window is not asked");
    assert_eq!(answers.counts(), (1, 0));
}

#[gpui_kit::test]
fn an_update_waits_for_a_yes_before_it_restarts_over_unsaved_edits(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (request, answers) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    assert!(cx.has_pending_prompt(), "the reader is asked");
    assert_eq!(answers.counts(), (0, 0), "the restart waits");
    cx.simulate_prompt_answer("Restart Anyway");
    settle(&shell, cx);
    assert_eq!(answers.counts(), (1, 0));
    assert!(cx.debug_bounds("notice").is_none(), "a yes adds no notice");
}

#[gpui_kit::test]
fn cancel_keeps_the_app_and_its_unsaved_edits_when_an_update_wants_to_restart(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (request, answers) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    cx.simulate_prompt_answer("Cancel");
    settle(&shell, cx);
    assert_eq!(answers.counts(), (0, 1));
    assert_eq!(shell.read_with(cx, |s, cx| s.unsaved(cx)), 1, "the edit is still there");
    assert!(cx.debug_bounds("notice").is_some(), "the reader is told the update waits");
}

#[gpui_kit::test]
fn a_restart_the_updater_asks_for_reaches_the_window_and_waits_on_unsaved_edits(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let (request, answers) = waiting();
    sender.unbounded_send(request).unwrap();
    settle(&shell, cx);
    assert!(cx.has_pending_prompt(), "the reader is asked");
    assert_eq!(answers.counts(), (0, 0));
    cx.simulate_prompt_answer("Cancel");
    settle(&shell, cx);
    assert_eq!(answers.counts(), (0, 1));
}

#[gpui_kit::test]
fn a_restart_the_updater_asks_for_goes_ahead_when_nothing_is_unsaved(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let (request, answers) = waiting();
    sender.unbounded_send(request).unwrap();
    settle(&shell, cx);
    assert!(!cx.has_pending_prompt());
    assert_eq!(answers.counts(), (1, 0));
}

#[gpui_kit::test]
fn the_updater_may_ask_for_a_restart_more_than_once_in_a_run(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let ((first, first_answers), (second, second_answers)) = (waiting(), waiting());
    sender.unbounded_send(first).unwrap();
    sender.unbounded_send(second).unwrap();
    settle(&shell, cx);
    assert_eq!((first_answers.counts(), second_answers.counts()), ((1, 0), (1, 0)));
}
