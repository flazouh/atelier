//! The Bots view of the shell: its place on the rail, the bots in its sidebar, and the profile of the one chosen.
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::super::{Shell, ShellView};
use super::{settle, with_a_session};

/// An app whose bots live in `root`, as a reader's live next to the settings file.
fn keeping_bots_in(shell: &Entity<Shell>, root: std::path::PathBuf, cx: &mut VisualTestContext) {
    shell.update(cx, |s, _| s.bots_root = Some(root));
}

#[gpui_kit::test]
fn a_press_on_bots_in_the_rail_lists_the_starters_and_shows_the_first_profile_and_a_row_switches_it(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    keeping_bots_in(&shell, dir.path().join("bots"), cx);
    assert!(cx.debug_bounds("bots-sidebar").is_none());
    let rail = cx.debug_bounds("rail-bots").expect("the rail has Bots");
    cx.simulate_click(rail.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Bots, "the view is a lens of the shell");
    assert!(cx.debug_bounds("bots-sidebar").is_some(), "the bots are in the sidebar");
    for row in [
        "bot-row-bolt", "bot-row-dot", "bot-row-gus", "bot-row-ink", "bot-row-mimi",
        "bot-row-nimbus", "bot-row-olive", "bot-row-pip", "bot-row-quill", "bot-row-skip",
    ] {
        assert!(cx.debug_bounds(row).is_some(), "{row} is drawn");
    }
    assert_eq!(std::fs::read_dir(dir.path().join("bots/bots")).unwrap().count(), 10, "the starters were seeded on disk");
    assert!(cx.debug_bounds("bot-profile-bolt").is_some(), "the first bot by id is the one shown");
    assert!(cx.debug_bounds("bot-mood-idle").is_some(), "the mood switch is under the face");
    assert!(cx.debug_bounds("bot-notes").is_some(), "the notes section is there");
    let dot = cx.debug_bounds("bot-row-dot").unwrap();
    cx.simulate_click(dot.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("bot-profile-dot").is_some(), "a press on a row shows that bot");
    assert!(cx.debug_bounds("bot-profile-bolt").is_none());
    let state = shell.read_with(cx, |s, cx| crate::control::state(s, cx));
    assert_eq!(state["view"], "bots", "the control socket says which lens is in front");
    assert_eq!(state["bots"]["chosen"], "dot", "and which bot is chosen");
    assert_eq!(state["bots"]["rows"].as_array().map(Vec::len), Some(10));
    assert!(state["bots"]["error"].is_null());
    let marked = shell.read_with(cx, |_, cx| crate::control::find("bot-row-dot", cx).is_some());
    assert!(marked, "a script presses a row by its name");
    let sessions = cx.debug_bounds("rail-sessions").expect("the rail has Sessions");
    cx.simulate_click(sessions.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert!(cx.debug_bounds("bot-profile").is_none(), "another lens leaves it");
    assert_eq!(ShellView::from_words(Some("bots")), ShellView::Bots, "the settings keep it by name");
}

#[gpui_kit::test]
fn a_module_opens_the_bots_lens_by_name(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    keeping_bots_in(&shell, dir.path().join("bots"), cx);
    shell.update(cx, |shell, cx| shell.open_view("bots", cx));
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Bots);
    assert!(cx.debug_bounds("bot-profile").is_some());
}

#[gpui_kit::test]
fn a_bot_file_that_does_not_parse_is_said_in_the_view_and_the_app_goes_on(cx: &mut TestAppContext) {
    let (shell, cx, dir) = with_a_session(cx, 1400.);
    let root = dir.path().join("bots");
    std::fs::create_dir_all(root.join("bots")).unwrap();
    std::fs::write(root.join("bots/bad.json"), "{").unwrap();
    keeping_bots_in(&shell, root, cx);
    shell.update(cx, |shell, cx| shell.open_view("bots", cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("bots-error").is_some(), "the view says the folder could not be read");
    assert!(cx.debug_bounds("bot-profile").is_none());
    let state = shell.read_with(cx, |s, cx| crate::control::state(s, cx));
    assert!(state["bots"]["error"].as_str().unwrap().contains("bad.json"), "the error names the file: {}", state["bots"]["error"]);
    assert!(state["bots"]["chosen"].is_null(), "nothing is chosen while nothing is read");
    let sessions = cx.debug_bounds("rail-sessions").unwrap();
    cx.simulate_click(sessions.center(), gpui_kit::Modifiers::default());
    settle(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.view), ShellView::Sessions, "the app goes on");
}

#[gpui_kit::test]
fn without_a_folder_the_view_says_so_and_writes_nothing(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    shell.update(cx, |s, _| s.bots_root = None);
    shell.update(cx, |shell, cx| shell.open_view("bots", cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("bots-empty").is_some());
}
