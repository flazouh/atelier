use std::{cell::Cell, rc::Rc};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::*;
use beui::theme::{Appearance, picked, set_appearance};

#[test]
fn the_mode_is_kept_by_its_key_and_a_stranger_is_refused() {
    for mode in Mode::ALL {
        assert_eq!(Mode::from_key(mode.key()), Some(mode));
    }
    assert_eq!(Mode::from_key("sepia"), None);
}

#[test]
fn every_offered_colour_is_a_distinct_name_and_colour() {
    let names: std::collections::HashSet<_> = PRIMARIES.iter().map(|p| p.0).collect();
    let colours: std::collections::HashSet<_> = PRIMARIES.iter().map(|p| p.1).collect();
    assert_eq!((names.len(), colours.len()), (PRIMARIES.len(), PRIMARIES.len()));
    assert!(!names.contains("default"), "\"default\" is the theme's own ink");
}

fn open<'a>(saved: &lathe_settings::Settings, cx: &'a mut TestAppContext) -> (Entity<SettingsPane>, &'a mut VisualTestContext, Rc<Cell<usize>>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_appearance(Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let closed = Rc::new(Cell::new(0));
    let counter = closed.clone();
    let saved = saved.clone();
    let agents = vec![AgentRow { name: "Claude Code".into(), models: vec!["Opus".into(), "Sonnet".into()] }, AgentRow { name: "lathe".into(), models: Vec::new() }];
    let (pane, cx) = cx.add_window_view(move |_, cx| SettingsPane::new(&saved, agents, cx));
    cx.update(|window, cx| {
        let sub = cx.subscribe(&pane, move |_, event: &SettingsEvent, _| {
            if matches!(event, SettingsEvent::Close) {
                counter.set(counter.get() + 1);
            }
        });
        std::mem::forget(sub);
        pane.read(cx).focus_handle(cx).focus(window, cx);
    });
    cx.simulate_resize(gpui_kit::size(px(900.), px(900.)));
    for _ in 0..3 {
        cx.run_until_parked();
        pane.update(cx, |_, cx| cx.notify());
    }
    cx.run_until_parked();
    (pane, cx, closed)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is not drawn")).center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    for _ in 0..3 {
        cx.run_until_parked();
    }
}

fn wait_for(path: &std::path::Path, ok: impl Fn(&lathe_settings::Settings) -> bool) -> lathe_settings::Settings {
    for _ in 0..200 {
        let now = lathe_settings::load(path);
        if ok(&now) {
            return now;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    lathe_settings::load(path)
}

/// One test, since the settings file is named by an environment variable the whole process shares.
#[gpui_kit::test]
fn a_pick_and_a_mode_apply_at_once_and_are_kept(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("lathe-settings-pane-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    // SAFETY: nothing else in this test binary reads or writes the variable while this test runs.
    unsafe { std::env::set_var("LATHE_SETTINGS", &file) };
    let (_pane, cx, closed) = open(&lathe_settings::Settings::default(), cx);

    // The default primary is the theme's ink.
    let (ink, page) = cx.update(|_, cx| (cx.theme().foreground, cx.theme().background));
    assert_eq!(cx.update(|_, cx| cx.theme().primary), ink);

    // Blue is the second swatch (the first is the default).
    click(cx, "color-swatch-1");
    let blue = colour(PRIMARIES[0].1);
    assert_eq!(cx.update(|_, cx| picked(cx)), Some(blue), "the pick is in force at once");
    let theme = cx.update(|_, cx| cx.theme().clone());
    assert_eq!(theme.primary, blue, "and it is the primary button's fill");
    assert!(theme.primary_foreground == page || theme.primary_foreground == ink);
    assert_eq!(wait_for(&file, |s| s.primary.is_some()).primary, Some(PRIMARIES[0].1), "and it is kept");

    // Back to the default.
    click(cx, "color-swatch-0");
    assert_eq!(cx.update(|_, cx| picked(cx)), None);
    assert_eq!(cx.update(|_, cx| cx.theme().primary), ink);
    assert_eq!(wait_for(&file, |s| s.primary.is_none() && s.mode.is_none()).primary, None);

    // Dark puts the dark theme in force, and the pick survives the change of theme.
    click(cx, "color-swatch-3");
    click(cx, "mode-dark");
    let dark = cx.update(|_, cx| cx.theme().clone());
    assert_eq!(dark.appearance, Appearance::Dark);
    assert_eq!(dark.primary, colour(PRIMARIES[2].1), "the pick is worn by the new theme");
    assert_eq!(wait_for(&file, |s| s.mode.as_deref() == Some("dark")).mode.as_deref(), Some("dark"));

    // Escape asks to close.
    cx.simulate_keystrokes("escape");
    assert_eq!(closed.get(), 1);
    unsafe { std::env::remove_var("LATHE_SETTINGS") };
}

#[gpui_kit::test]
fn the_pane_opens_on_what_was_kept_and_lists_the_agents_and_the_keys(cx: &mut TestAppContext) {
    let saved = lathe_settings::Settings { mode: Some("dark".into()), primary: Some(PRIMARIES[3].1), ..Default::default() };
    let (pane, cx, _) = open(&saved, cx);
    pane.read_with(cx, |p, _| {
        assert_eq!(p.mode, Mode::Dark);
        assert_eq!(p.primary.as_ref(), "red");
        assert_eq!(p.agents.len(), 2);
    });
}

fn wheel(cx: &mut VisualTestContext, dy: f32) {
    cx.simulate_event(gpui_kit::ScrollWheelEvent {
        position: gpui_kit::point(px(450.), px(200.)),
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(dy))),
        modifiers: gpui_kit::Modifiers::default(),
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    for _ in 0..3 {
        cx.run_until_parked();
    }
}

/// One test, since the settings file is named by an environment variable the whole process shares.
#[gpui_kit::test]
fn a_short_window_scrolls_to_the_last_agent_and_the_close_button_closes_the_pane(cx: &mut TestAppContext) {
    let (_, cx, closed) = open(&lathe_settings::Settings::default(), cx);
    cx.simulate_resize(gpui_kit::size(px(900.), px(500.)));
    for _ in 0..3 {
        cx.run_until_parked();
    }
    let window = 500.;
    let last = cx.debug_bounds("agent-row-1").expect("the last agent row is drawn");
    assert!(f32::from(last.bottom()) > window, "at 500 px tall the last row starts off screen: {last:?}");
    wheel(cx, -4000.);
    let last = cx.debug_bounds("agent-row-1").expect("still drawn");
    assert!(f32::from(last.top()) >= 0. && f32::from(last.bottom()) <= window, "after the wheel it is on screen: {last:?}");
    wheel(cx, 4000.);
    let first = cx.debug_bounds("agent-row-0").expect("drawn");
    assert!(f32::from(first.top()) > window, "and it scrolls back up");

    let close = cx.debug_bounds("settings-close").expect("a close button is drawn");
    assert!(f32::from(close.right()) > 900. - 40. && f32::from(close.top()) < 40., "at the top right: {close:?}");
    assert_eq!(closed.get(), 0);
    click(cx, "settings-close");
    assert_eq!(closed.get(), 1, "a click on it closes the pane");
}

/// The task rules show as switches, on unless the reader turned them off, and a switch changes the set.
#[gpui_kit::test]
fn the_task_rules_show_as_switches_and_a_switch_changes_the_set(cx: &mut TestAppContext) {
    let saved = lathe_settings::Settings { task_rules_off: vec!["merge".into()], ..Default::default() };
    let (pane, cx, _) = open(&saved, cx);
    for rule in lathe_tracker::Rule::ALL {
        assert!(cx.debug_bounds(super::rule_switch(rule)).is_some(), "{} has its switch", rule.id());
    }
    let on = |cx: &mut VisualTestContext, rule| pane.read_with(cx, |p, _| p.rules.is_on(rule));
    assert!(on(cx, lathe_tracker::Rule::SessionStartMovesToInProgress));
    assert!(!on(cx, lathe_tracker::Rule::MergeMovesToDone), "kept off");
    pane.update(cx, |p, _| p.rules.set(lathe_tracker::Rule::MergeMovesToDone, true));
    assert!(on(cx, lathe_tracker::Rule::MergeMovesToDone));
}
