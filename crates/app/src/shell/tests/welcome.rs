use gpui_kit::{Entity, Modifiers, TestAppContext, VisualTestContext};

use super::{open_shell_with, settle};
use crate::shell::{Shell, welcome::owed};

/// A shell as the app opens it: built from the settings, then asked to welcome.
fn start(cx: &mut TestAppContext, saved: atelier_settings::Settings) -> (Entity<Shell>, &mut VisualTestContext) {
    let (shell, cx) = open_shell_with(cx, saved.clone());
    shell.update(cx, |s, cx| s.welcome_at_start(&saved, cx));
    settle(&shell, cx);
    (shell, cx)
}

#[test]
fn a_reader_is_owed_the_page_until_the_settings_say_they_pressed_its_button() {
    let fresh = atelier_settings::Settings::default();
    assert!(owed(&fresh), "no settings: a first start");
    assert!(owed(&atelier_settings::Settings { welcomed: Some(false), ..Default::default() }));
    assert!(!owed(&atelier_settings::Settings { welcomed: Some(true), ..Default::default() }));
}

#[test]
fn the_settings_keep_that_the_page_was_seen_and_an_older_file_without_the_key_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{ "theme": "atelier Dark" }"#).unwrap();
    assert!(owed(&atelier_settings::load(&path)), "a file from a version before the page");
    atelier_settings::update(&path, |s| s.welcomed = Some(true)).unwrap();
    let saved = atelier_settings::load(&path);
    assert!(!owed(&saved));
    assert_eq!(saved.theme.as_deref(), Some("atelier Dark"), "the rest of the file is kept");
}

#[gpui_kit::test]
fn a_first_start_covers_the_whole_window_with_the_welcome_page(cx: &mut TestAppContext) {
    let (shell, cx) = start(cx, atelier_settings::Settings::default());
    assert!(shell.read_with(cx, |s, _| s.welcome_shown()));
    let cover = cx.debug_bounds("welcome").expect("the cover is drawn");
    let page = cx.debug_bounds("welcome-page").expect("the page is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert_eq!((cover.origin.x, cover.origin.y, cover.size), (gpui_kit::px(0.), gpui_kit::px(0.), window), "the cover is the window");
    assert_eq!(page.size, window, "the page fills it, the title bar too");
    assert!(cx.debug_bounds("welcome-continue").is_some(), "its button is drawn");
}

#[gpui_kit::test]
fn pressing_the_button_takes_the_page_away_and_the_start_screen_answers_again(cx: &mut TestAppContext) {
    let (shell, cx) = start(cx, atelier_settings::Settings::default());
    // Open over SSH opens the app's own dialog at once; Open Folder asks the system's first.
    let remote = cx.debug_bounds("open-remote").expect("the start screen is drawn under the page");
    cx.simulate_click(remote.center(), Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("open-over-ssh").is_none(), "a press does not go through the page");
    let button = cx.debug_bounds("welcome-continue").expect("the button is drawn");
    cx.simulate_click(button.center(), Modifiers::default());
    settle(&shell, cx);
    assert!(!shell.read_with(cx, |s, _| s.welcome_shown()));
    assert!(cx.debug_bounds("welcome").is_none(), "the page is gone");
    cx.simulate_click(remote.center(), Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("open-over-ssh").is_some(), "the start screen answers again");
    shell.update(cx, |s, cx| s.dismiss_welcome(cx));
    settle(&shell, cx);
    assert!(!shell.read_with(cx, |s, _| s.welcome_shown()), "a second dismissal changes nothing");
}

#[gpui_kit::test]
fn a_reader_who_pressed_the_button_before_does_not_see_the_page_again(cx: &mut TestAppContext) {
    let (shell, cx) = start(cx, atelier_settings::Settings { welcomed: Some(true), ..Default::default() });
    assert!(!shell.read_with(cx, |s, _| s.welcome_shown()));
    assert!(cx.debug_bounds("welcome").is_none());
}

#[gpui_kit::test]
fn a_kept_changelog_waits_under_the_page_and_opens_once_the_button_is_pressed(cx: &mut TestAppContext) {
    let kept = atelier_settings::WhatsNew { version: env!("CARGO_PKG_VERSION").into(), notes: "## What is new\n\n- **Fast:** it is faster".into() };
    let (shell, cx) = start(cx, atelier_settings::Settings { whats_new: Some(kept), ..Default::default() });
    assert!(cx.debug_bounds("welcome").is_some(), "the page shows first");
    assert!(cx.debug_bounds("release-sheet").is_none(), "alone: no changelog over it");
    shell.update(cx, |s, cx| s.dismiss_welcome(cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("release-sheet").is_some(), "the changelog opens after the press");
}

#[gpui_kit::test]
fn the_control_socket_says_whether_the_page_shows(cx: &mut TestAppContext) {
    let (shell, cx) = start(cx, atelier_settings::Settings::default());
    assert_eq!(shell.read_with(cx, |s, cx| crate::control::state(s, cx))["welcome"], true);
    shell.update(cx, |s, cx| s.dismiss_welcome(cx));
    assert_eq!(shell.read_with(cx, |s, cx| crate::control::state(s, cx))["welcome"], false);
}
