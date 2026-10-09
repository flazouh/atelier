use std::rc::Rc;

use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::{open_shell, open_shell_with, settle, with_a_session};
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

/// Whether the element marked `name` was drawn on the last frame.
fn drawn(cx: &mut VisualTestContext, name: String) -> bool {
    cx.debug_bounds(Box::leak(name.into_boxed_str())).is_some()
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
fn an_update_the_daily_look_finds_shows_its_percentage_and_then_a_button_and_opens_no_sheet(cx: &mut TestAppContext) {
    let (shell, cx, _calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Downloading { fraction: 0.5 });
    assert!(cx.debug_bounds("update-progress").is_some(), "the download shows a percentage");
    assert!(cx.debug_bounds("update-chip").is_none() && cx.debug_bounds("release-sheet").is_none() && cx.debug_bounds("notice").is_none(), "and nothing else");
    tell(&shell, cx, UpdateEvent::Extracting { fraction: 1. });
    tell(&shell, cx, UpdateEvent::Ready);
    assert!(cx.debug_bounds("update-chip").is_some(), "a button says the update is ready");
    assert!(cx.debug_bounds("update-progress").is_none() && cx.debug_bounds("release-sheet").is_none(), "no sheet opens by itself");
}

#[gpui_kit::test]
fn the_ready_button_names_the_version_and_a_press_installs_the_update_with_no_sheet(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    let chip = cx.debug_bounds("update-chip").expect("the button is there");
    cx.simulate_click(chip.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(calls.counts(), (0, 0, 1, 0), "the updater was told to install, and not to wait");
    assert!(cx.debug_bounds("release-sheet").is_none(), "no sheet opened");
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Installing);
    assert!(cx.debug_bounds("update-chip").is_none(), "the button is gone: it says Installing instead");
}

#[gpui_kit::test]
fn a_press_on_the_button_with_no_update_ready_does_nothing(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    shell.update(cx, |s, cx| s.update_install(cx));
    assert_eq!(calls.counts(), (0, 0, 0, 0));
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Idle);
}

#[gpui_kit::test]
fn quitting_with_an_update_ready_that_was_never_pressed_installs_it_at_quit(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    let (_sender, events) = futures_channel::mpsc::unbounded();
    shell.update_in(cx, |s, window, cx| s.serve_updates(events, window, cx));
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    assert_eq!(calls.counts(), (0, 0, 0, 0), "nothing yet");
    // What the platform does when the app quits: it runs every quit observer.
    cx.cx.update(|cx| cx.shutdown());
    assert_eq!(calls.counts(), (0, 0, 0, 1), "the updater was told to install at quit, once");
}

#[gpui_kit::test]
fn quitting_with_no_update_ready_or_one_already_installing_tells_the_updater_nothing(cx: &mut TestAppContext) {
    for events in [vec![], vec![UpdateEvent::Checking { user: false }, UpdateEvent::Installing]] {
        let (shell, cx, calls, _dir) = with_an_updater(cx);
        shell.update(cx, |s, cx| {
            for event in events {
                s.update_event(event, cx);
            }
            s.update_at_quit(cx);
        });
        assert_eq!(calls.counts(), (0, 0, 0, 0));
    }
}

#[gpui_kit::test]
fn a_look_the_reader_asked_for_opens_no_sheet_and_says_when_nothing_is_newer(cx: &mut TestAppContext) {
    let (shell, cx, _calls, _dir) = with_an_updater(cx);
    ask_for_updates(&shell, cx);
    tell(&shell, cx, UpdateEvent::UpToDate);
    assert_eq!(shell.read_with(cx, |s, _| s.update.clone()), UpdateState::Idle);
    ask_for_updates(&shell, cx);
    tell(&shell, cx, UpdateEvent::Found { version: "0.2.0".into(), notes: None, user: true });
    tell(&shell, cx, UpdateEvent::Ready);
    assert!(cx.debug_bounds("release-sheet").is_none(), "the update waits behind its button");
    assert!(cx.debug_bounds("update-chip").is_some());
}

#[gpui_kit::test]
fn asking_again_while_an_update_is_ready_says_so_and_does_not_look_again(cx: &mut TestAppContext) {
    let (shell, cx, calls, _dir) = with_an_updater(cx);
    found_by_the_daily_look(&shell, cx);
    tell(&shell, cx, UpdateEvent::Ready);
    ask_for_updates(&shell, cx);
    assert!(cx.debug_bounds("notice").is_some(), "a notice points to the button");
    assert!(cx.debug_bounds("release-sheet").is_none());
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

fn kept(version: &str) -> atelier_settings::Settings {
    atelier_settings::Settings {
        whats_new: Some(atelier_settings::WhatsNew { version: version.into(), notes: "## What is new\n\n- **Fast:** it is faster".into() }),
        ..Default::default()
    }
}

/// What the title bar and the sheet show for `kept`, the changelog the settings kept.
#[gpui_kit::test]
fn the_first_start_of_an_updated_version_opens_the_changelog_by_itself_and_close_forgets_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell_with(cx, kept(env!("CARGO_PKG_VERSION")));
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some(), "the sheet is open at start");
    assert!(cx.debug_bounds("whats-new-chip").is_none(), "there is no chip any more");
    assert!(cx.debug_bounds("release-install").is_none() && cx.debug_bounds("release-later").is_none(), "and no buttons but Close");
    assert!(cx.debug_bounds("release-earlier-0").is_none(), "the old layout is gone");
    assert!(cx.debug_bounds("release-0").is_some() && cx.debug_bounds("release-1").is_some(), "the new version first, the older ones under it");
    let close = cx.debug_bounds("release-close").expect("Close is there");
    cx.simulate_click(close.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_none(), "Close closes it");
    assert!(shell.read_with(cx, |s, _| s.whats_new.is_none()), "and the kept record is forgotten");
}

#[gpui_kit::test]
fn the_second_start_does_not_open_the_sheet_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    atelier_settings::update(&path, |s| *s = kept(env!("CARGO_PKG_VERSION"))).unwrap();
    let saved = atelier_settings::load(&path);
    let (first, cx) = open_shell_with(cx, saved);
    assert!(first.read_with(cx, |s, _| s.whats_new_open), "the first start opens it");
    // Closing writes the settings without the record, as the app does off the UI thread.
    atelier_settings::update(&path, |s| s.whats_new = None).unwrap();
    let saved = atelier_settings::load(&path);
    assert!(saved.whats_new.is_none());
    let (second, cx) = open_shell_with(cx, saved);
    assert!(!second.read_with(cx, |s, _| s.whats_new_open), "the second start does not");
    assert!(cx.debug_bounds("release-sheet").is_none());
}

#[gpui_kit::test]
fn escape_closes_the_whats_new_sheet_too(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell_with(cx, kept(env!("CARGO_PKG_VERSION")));
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some());
    cx.simulate_keystrokes("escape");
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_none() && shell.read_with(cx, |s, _| s.whats_new.is_none()));
}

#[gpui_kit::test]
fn the_sheet_of_an_update_shows_the_new_version_first_with_its_date_and_the_older_releases_under_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell_with(cx, atelier_settings::Settings::default());
    shell.update(cx, |s, cx| {
        s.whats_new = Some(atelier_settings::WhatsNew {
            version: "0.1.5".into(),
            notes: "## What is new in 0.1.5\nReleased: 2026-10-08\n- **Fast:** it is faster".into(),
        });
        s.whats_new_open = true;
        cx.notify();
    });
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some());
    assert!(cx.debug_bounds("release-date-0").is_some(), "the new version has its date");
    let older = crate::changelog::releases().iter().filter(|(v, _)| crate::updater::is_older(v, "0.1.5")).count();
    assert_eq!(older, (1..=older).filter(|n| drawn(cx, format!("release-{n}"))).count(), "every older release is listed");
    assert!(!drawn(cx, format!("release-{}", older + 1)), "and no newer one");
}

#[gpui_kit::test]
fn a_changelog_kept_for_another_version_shows_no_chip(cx: &mut TestAppContext) {
    for version in ["99.0.0", "0.0.1"] {
        let (shell, cx) = open_shell_with(cx, kept(version));
        settle(&shell, cx);
        assert!(cx.debug_bounds("whats-new-chip").is_none(), "{version}: not this version");
    }
}

#[gpui_kit::test]
fn the_version_in_the_status_bar_opens_the_changelog_with_every_release_and_a_date_and_close_closes_it(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = super::with_a_session(cx, 1200.);
    settle(&shell, cx);
    let button = cx.debug_bounds("status-version").expect("the version is on the screen");
    assert!(cx.debug_bounds("release-sheet").is_none());
    cx.simulate_click(button.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some(), "a press opens the changelog");
    assert!(cx.debug_bounds("release-install").is_none(), "it has nothing to restart");
    for (at, (version, markdown)) in crate::changelog::releases().iter().enumerate() {
        assert!(drawn(cx, format!("release-{at}")), "{version} is listed, newest first");
        assert_eq!(drawn(cx, format!("release-date-{at}")), crate::updater::release_date(markdown).is_some(), "{version}: a date when its notes have one");
    }
    assert!(cx.debug_bounds("release-date-0").is_some(), "the current release has its date");
    cx.simulate_keystrokes("escape");
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_none(), "Escape closes it");
}
