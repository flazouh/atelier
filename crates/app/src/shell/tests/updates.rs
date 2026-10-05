use std::{cell::Cell, rc::Rc};

use gpui_kit::{TestAppContext, VisualTestContext};

use super::{open_shell, settle, with_a_session};
use crate::{
    shell::{CheckForUpdates, Shell},
    updater::{NoDriver, RelaunchRequest, RequestSender, UpdateDriver, Updater},
};

/// A driver that counts the checks it is asked for.
struct Counting(Rc<Cell<u32>>);

impl UpdateDriver for Counting {
    fn available(&self) -> bool {
        true
    }

    fn check(&self) {
        self.0.set(self.0.get() + 1);
    }
}

/// A restart the updater waits on, with counters for the answer it got.
struct Waiting {
    proceeded: Rc<Cell<u32>>,
    declined: Rc<Cell<u32>>,
}

impl RelaunchRequest for Waiting {
    fn proceed(self: Box<Self>) {
        self.proceeded.set(self.proceeded.get() + 1);
    }

    fn decline(self: Box<Self>) {
        self.declined.set(self.declined.get() + 1);
    }
}

/// The request, and the counters of the restarts it was allowed and refused.
type Answers = (Box<dyn RelaunchRequest>, Rc<Cell<u32>>, Rc<Cell<u32>>);

fn waiting() -> Answers {
    let (proceeded, declined) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    (Box::new(Waiting { proceeded: proceeded.clone(), declined: declined.clone() }), proceeded, declined)
}

fn ask_for_updates(shell: &gpui_kit::Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update_in(cx, |s, window, cx| s.check_for_updates(&CheckForUpdates, window, cx));
    settle(shell, cx);
}

/// Makes the one open tab hold an edit that is not saved.
fn leave_an_edit_unsaved(shell: &gpui_kit::Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update_in(cx, |s, window, cx| {
        s.active().unwrap().clone().update(cx, |p, cx| p.open_file("a.txt", window, cx));
    });
    settle(shell, cx);
    shell.update_in(cx, |s, _, cx| {
        s.active().unwrap().clone().update(cx, |p, _| p.buffers.get_mut("a.txt").unwrap().dirty = true);
    });
    assert_eq!(shell.read_with(cx, |s, cx| s.unsaved(cx)), 1);
}

#[gpui_kit::test]
fn asking_for_updates_in_a_build_that_cannot_update_says_so_in_a_notice(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    cx.update(|_, cx| cx.set_global(Updater::new(Rc::new(NoDriver))));
    ask_for_updates(&shell, cx);
    assert!(cx.debug_bounds("notice").is_some(), "the notice shows");
}

#[gpui_kit::test]
fn asking_for_updates_hands_the_check_to_the_updater_and_adds_no_notice(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let checks = Rc::new(Cell::new(0));
    cx.update(|_, cx| cx.set_global(Updater::new(Rc::new(Counting(checks.clone())))));
    ask_for_updates(&shell, cx);
    assert_eq!(checks.get(), 1);
    assert!(cx.debug_bounds("notice").is_none(), "the updater shows its own window");
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
    let (request, proceeded, declined) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    assert!(!cx.has_pending_prompt(), "a clean window is not asked");
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[gpui_kit::test]
fn an_update_waits_for_a_yes_before_it_restarts_over_unsaved_edits(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (request, proceeded, declined) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    assert!(cx.has_pending_prompt(), "the reader is asked");
    assert_eq!((proceeded.get(), declined.get()), (0, 0), "the restart waits");
    cx.simulate_prompt_answer("Restart Anyway");
    settle(&shell, cx);
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[gpui_kit::test]
fn cancel_keeps_the_app_and_its_unsaved_edits_when_an_update_wants_to_restart(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (request, proceeded, declined) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    cx.simulate_prompt_answer("Cancel");
    settle(&shell, cx);
    assert_eq!((proceeded.get(), declined.get()), (0, 1));
    assert_eq!(shell.read_with(cx, |s, cx| s.unsaved(cx)), 1, "the edit is still there");
    assert!(cx.debug_bounds("notice").is_some(), "the reader is told the update waits");
}

#[gpui_kit::test]
fn a_yes_to_the_restart_adds_no_notice(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (request, _, _) = waiting();
    shell.update_in(cx, |s, window, cx| s.relaunch_for_update(request, window, cx));
    settle(&shell, cx);
    cx.simulate_prompt_answer("Restart Anyway");
    settle(&shell, cx);
    assert!(cx.debug_bounds("notice").is_none());
}

fn requests() -> (RequestSender, crate::updater::Requests) {
    futures_channel::mpsc::unbounded()
}

#[gpui_kit::test]
fn a_restart_the_updater_asks_for_reaches_the_window_and_waits_on_unsaved_edits(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    leave_an_edit_unsaved(&shell, cx);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let (request, proceeded, declined) = waiting();
    sender.unbounded_send(request).unwrap();
    settle(&shell, cx);
    assert!(cx.has_pending_prompt(), "the reader is asked");
    assert_eq!((proceeded.get(), declined.get()), (0, 0));
    cx.simulate_prompt_answer("Cancel");
    settle(&shell, cx);
    assert_eq!((proceeded.get(), declined.get()), (0, 1));
}

#[gpui_kit::test]
fn a_restart_the_updater_asks_for_goes_ahead_when_nothing_is_unsaved(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let (request, proceeded, declined) = waiting();
    sender.unbounded_send(request).unwrap();
    settle(&shell, cx);
    assert!(!cx.has_pending_prompt());
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[gpui_kit::test]
fn the_updater_may_ask_for_a_restart_more_than_once_in_a_run(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let (sender, receiver) = requests();
    shell.update_in(cx, |s, window, cx| s.serve_relaunches(receiver, window, cx));
    let (first, proceeded, _) = waiting();
    let (second, also_proceeded, _) = waiting();
    sender.unbounded_send(first).unwrap();
    sender.unbounded_send(second).unwrap();
    settle(&shell, cx);
    assert_eq!((proceeded.get(), also_proceeded.get()), (1, 1));
}
