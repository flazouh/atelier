use gpui_kit::Focusable;
use atelier_ui::ActiveTheme;
use std::{cell::Cell, rc::Rc};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};

use super::*;
use atelier_ui::theme::{Appearance, picked, set_appearance};

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

fn open<'a>(saved: &atelier_settings::Settings, cx: &'a mut TestAppContext) -> (Entity<SettingsPane>, &'a mut VisualTestContext, Rc<Cell<usize>>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        set_appearance(Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let closed = Rc::new(Cell::new(0));
    let counter = closed.clone();
    let saved = saved.clone();
    let agents = vec![AgentRow { name: "Claude Code".into(), mark: None, backend: "claude-code".into() }, AgentRow { name: "atelier".into(), mark: None, backend: "atelier".into() }];
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

fn wait_for(path: &std::path::Path, ok: impl Fn(&atelier_settings::Settings) -> bool) -> atelier_settings::Settings {
    for _ in 0..200 {
        let now = atelier_settings::load(path);
        if ok(&now) {
            return now;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    atelier_settings::load(path)
}

/// One test, since the settings file is named by an environment variable the whole process shares.
#[gpui_kit::test]
fn a_pick_and_a_mode_apply_at_once_and_are_kept(cx: &mut TestAppContext) {
    let dir = crate::test_dirs::path();
    let file = dir.join("settings.json");
    // SAFETY: nothing else in this test binary reads or writes the variable while this test runs.
    unsafe { std::env::set_var("ATELIER_SETTINGS", &file) };
    let (_pane, cx, closed) = open(&atelier_settings::Settings::default(), cx);

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

    // A switch in Agents makes a picked skill run at once, and it is kept.
    let runs = |cx: &mut VisualTestContext| cx.update(|_, cx| crate::agent_session::runs_picked_skills(cx));
    assert!(!runs(cx), "a picked skill waits by default");
    click(cx, "section-agents");
    click(cx, "skills-run-when-picked");
    assert!(runs(cx));
    assert_eq!(wait_for(&file, |s| s.run_picked_skills.is_some()).run_picked_skills, Some(true));

    // The tool-call density is a choice of three in Agents: kept, and in force at once.
    let density = |cx: &mut VisualTestContext| cx.update(|_, cx| crate::tool_density::tool_density(cx));
    assert_eq!(density(cx), crate::tool_density::ToolDensity::Grouped, "grouped by default");
    click(cx, "tool-density-lines");
    assert_eq!(density(cx), crate::tool_density::ToolDensity::Lines);
    assert_eq!(wait_for(&file, |s| s.tool_density.is_some()).tool_density.as_deref(), Some("lines"));
    click(cx, "tool-density-detailed");
    assert_eq!(density(cx), crate::tool_density::ToolDensity::Detailed);
    assert_eq!(wait_for(&file, |s| s.tool_density.as_deref() == Some("detailed")).tool_density.as_deref(), Some("detailed"));

    // The language is a choice in Appearance: a language is kept by its tag, and the first entry hands the choice back to
    // the system. English is the one picked here, so the words other tests read do not change under them.
    click(cx, "section-appearance");
    assert!(cx.debug_bounds("language").is_some(), "the language is offered");
    let english = 1 + atelier_i18n::Locale::ALL.iter().position(|l| *l == atelier_i18n::Locale::En).unwrap();
    _pane.update(cx, |pane, cx| pane.choose_language(english, cx));
    cx.run_until_parked();
    assert_eq!(wait_for(&file, |s| s.language.is_some()).language.as_deref(), Some("en"));
    _pane.update(cx, |pane, cx| pane.choose_language(0, cx));
    cx.run_until_parked();
    assert_eq!(wait_for(&file, |s| s.language.is_none()).language, None, "the system's again");

    // Connecting Linear keeps the key in the keychain and, in the file, only that Linear is connected and who it is.
    {
        use atelier_settings::secrets::{InMemory, LINEAR_KEY, Secrets};
        let secrets = std::sync::Arc::new(InMemory::default());
        cx.update(|_, cx| {
            cx.set_global(crate::accounts::AccountServices {
                secrets: secrets.clone(),
                linear: |_| Ok(std::sync::Arc::new(atelier_capabilities::tasks::MemoryTasks::new("acme"))),
                github: |_| Err(atelier_capabilities::CapError::Offline), ..crate::accounts::AccountServices::isolated()
})
        });
        _pane.update(cx, |pane, cx| pane.save_linear("lin_api_SECRET".into(), cx));
        cx.run_until_parked();
        let kept = wait_for(&file, |s| s.accounts.linear.is_some());
        assert_eq!(kept.accounts.linear.and_then(|linear| linear.person), Some("Me".into()));
        assert!(!std::fs::read_to_string(&file).unwrap().contains("lin_api_SECRET"), "the key is not in the settings file");
        assert_eq!(secrets.read(LINEAR_KEY).unwrap().as_deref(), Some("lin_api_SECRET"));
        _pane.update(cx, |pane, cx| pane.forget_linear(cx));
        cx.run_until_parked();
        assert_eq!(wait_for(&file, |s| s.accounts.linear.is_none()).accounts.linear, None, "forgetting clears the file's entry");
        assert_eq!(secrets.read(LINEAR_KEY).unwrap(), None);
    }

    // Escape asks to close.
    cx.simulate_keystrokes("escape");
    assert_eq!(closed.get(), 1);
    unsafe { std::env::remove_var("ATELIER_SETTINGS") };
}

#[gpui_kit::test]
fn the_pane_opens_on_what_was_kept_and_lists_the_agents_and_the_keys(cx: &mut TestAppContext) {
    let saved = atelier_settings::Settings { mode: Some("dark".into()), primary: Some(PRIMARIES[3].1), ..Default::default() };
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

/// A short window scrolls the long Keys list to its last row and back.
#[gpui_kit::test]
fn a_short_window_scrolls_the_keys_list(cx: &mut TestAppContext) {
    let (pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    pane.update(cx, |p, cx| p.show(Section::Keys, cx));
    cx.simulate_resize(gpui_kit::size(px(900.), px(300.)));
    for _ in 0..3 {
        cx.run_until_parked();
    }
    let window = 300.;
    let scroll = cx.debug_bounds("settings-scroll").expect("the scroll area is drawn");
    assert!(f32::from(scroll.bottom()) <= window + 1.);
    wheel(cx, -4000.);
    wheel(cx, 4000.);
}

/// The sections are listed at the left, one shows at a time, and a press on an entry shows its section.
#[gpui_kit::test]
fn a_press_on_a_section_shows_it_alone(cx: &mut TestAppContext) {
    let (pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    for section in Section::ALL {
        assert!(cx.debug_bounds(section.entry()).is_some(), "{} is listed", section.words());
    }
    assert!(cx.debug_bounds("mode-dark").is_some() && cx.debug_bounds("agent-row-0").is_none(), "Appearance first, with no agents");
    click(cx, "section-agents");
    assert!(cx.debug_bounds("agent-row-0").is_some() && cx.debug_bounds("mode-dark").is_none(), "Agents shows its rows and Appearance goes");
    assert!(cx.debug_bounds("select-monogram-C").is_some(), "an agent with no mark leads with its first letter");
    click(cx, "section-tasks");
    assert!(cx.debug_bounds(super::rule_switch(atelier_tracker::Rule::MergeMovesToDone)).is_some(), "Tasks shows its switches");
    assert!(cx.debug_bounds("design-tabs-0").is_none() && cx.debug_bounds("design-elevation-0").is_none(), "no design preview anywhere");
    pane.read_with(cx, |p, _| assert_eq!(p.section, Section::Tasks));
}

/// The task rules show as switches, on unless the reader turned them off, and a switch changes the set.
#[gpui_kit::test]
fn the_task_rules_show_as_switches_and_a_switch_changes_the_set(cx: &mut TestAppContext) {
    let saved = atelier_settings::Settings { task_rules_off: vec!["merge".into()], ..Default::default() };
    let (pane, cx, _) = open(&saved, cx);
    pane.update(cx, |p, cx| p.show(Section::Tasks, cx));
    for rule in atelier_tracker::Rule::ALL {
        assert!(cx.debug_bounds(super::rule_switch(rule)).is_some(), "{} has its switch", rule.id());
    }
    let on = |cx: &mut VisualTestContext, rule| pane.read_with(cx, |p, _| p.rules.is_on(rule));
    assert!(on(cx, atelier_tracker::Rule::SessionStartMovesToInProgress));
    assert!(!on(cx, atelier_tracker::Rule::MergeMovesToDone), "kept off");
    pane.update(cx, |p, _| p.rules.set(atelier_tracker::Rule::MergeMovesToDone, true));
    assert!(on(cx, atelier_tracker::Rule::MergeMovesToDone));
}

/// The Sidebar section holds the layout's knobs; a change reaches the shell as one event and is kept in the settings.
#[gpui_kit::test]
fn the_sidebar_section_edits_the_layout_and_says_so_once(cx: &mut TestAppContext) {
    use atelier_ui::sidebar_layout::BadgeShow;
    let (pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    let heard = Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        let sub = cx.subscribe(&pane, move |_, event: &SettingsEvent, _| {
            if let SettingsEvent::Sidebar(look) = event {
                log.borrow_mut().push(*look);
            }
        });
        std::mem::forget(sub);
    });
    click(cx, "section-sidebar");
    for name in ["badge-auto", "badge-always", "badge-never", "sidebar-time", "sidebar-icon", "fold-3", "fold-12", "earlier-5", "earlier-20"] {
        assert!(cx.debug_bounds(name).is_some(), "{name} is on the page");
    }
    click(cx, "badge-always");
    click(cx, "fold-12");
    let looks = heard.borrow().clone();
    assert_eq!(looks.len(), 2, "one event for each change");
    assert_eq!((looks[1].project_badge, looks[1].fold_after), (BadgeShow::Always, 12));
    assert_eq!(pane.read_with(cx, |p, _| p.look.project_badge), BadgeShow::Always);
}

/// The interface font size is the zoom, told in points of the body text; a choice reaches the shell as one event.
#[gpui_kit::test]
fn the_interface_font_size_is_the_zoom_and_says_so_once(cx: &mut TestAppContext) {
    let (pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    assert!(cx.debug_bounds("font-size").is_some(), "the slider is in Appearance");
    assert_eq!(super::helpers::font_size_words(1.), "14 pt");
    assert_eq!(super::helpers::font_size_words(1.2), "17 pt");
    let heard = Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = heard.clone();
    cx.update(|_, cx| {
        let sub = cx.subscribe(&pane, move |_, event: &SettingsEvent, _| {
            if let SettingsEvent::Zoom(zoom) = event {
                log.borrow_mut().push(*zoom);
            }
        });
        std::mem::forget(sub);
    });
    pane.update(cx, |p, cx| p.preview_zoom(1.8, cx));
    assert!(heard.borrow().is_empty(), "a drag in progress does not zoom the window under the pointer");
    assert_eq!(pane.read_with(cx, |p, _| p.shown_zoom()), 1.8, "but the slider and its label follow it");
    pane.update(cx, |p, cx| p.choose_zoom(1.3, cx));
    assert_eq!(heard.borrow().as_slice(), &[1.3]);
    assert_eq!(pane.read_with(cx, |p, _| p.zoom_preview), None, "the release applies it and the preview is gone");
}

#[gpui_kit::test]
fn the_dictation_section_sets_the_key_and_hold_at_once(cx: &mut TestAppContext) {
    let (_pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    click(cx, "section-dictation");
    assert!(cx.debug_bounds("dictation-mic").is_some(), "the microphone is chosen here too");
    use atelier_voice::hotkey::Key;
    assert_eq!(cx.update(|_, cx| crate::agent_session::dictation::prefs(cx).key), Key::DEFAULT);

    let (button, key) = if cfg!(target_os = "macos") { ("dictation-key-right-option", Key::RightOption) } else { ("dictation-key-alt", Key::Alt) };
    click(cx, button);
    assert_eq!(cx.update(|_, cx| crate::agent_session::dictation::prefs(cx).key), Some(key));
    click(cx, "dictation-key-off");
    assert_eq!(cx.update(|_, cx| crate::agent_session::dictation::prefs(cx).key), None);

    click(cx, "dictation-hold");
    assert!(cx.update(|_, cx| crate::agent_session::dictation::prefs(cx).hold));
}

/// Pull requests lists a switch for each part of a chip's card; a switch hides the part on every card, as
/// the card's own menu does, and the page shows what the menu changed.
#[gpui_kit::test]
fn the_card_parts_show_as_switches_that_hide_them_on_every_card(cx: &mut TestAppContext) {
    let (pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    pane.update(cx, |p, cx| p.show(Section::PullRequests, cx));
    cx.run_until_parked();
    for part in atelier_ui::PrPart::ALL {
        assert!(cx.debug_bounds(super::helpers::card_part_switch(part)).is_some(), "{} has its switch", part.key());
    }
    let shows = |cx: &mut VisualTestContext, part| cx.update(|_, cx| atelier_ui::pr_cards(cx).read(cx).parts().shows(part));
    assert!(shows(cx, atelier_ui::PrPart::Files));
    click(cx, super::helpers::card_part_switch(atelier_ui::PrPart::Files));
    assert!(!shows(cx, atelier_ui::PrPart::Files), "the switch hid the files");
    assert!(shows(cx, atelier_ui::PrPart::Reviewers), "and nothing else");
}

/// While the mode is System, the app turns light or dark as the system does; a mode the reader picked stays.
#[gpui_kit::test]
fn the_system_mode_follows_a_change_of_the_systems_appearance(cx: &mut gpui_kit::TestAppContext) {
    use atelier_ui::theme::{ActiveTheme, Appearance};
    use gpui_kit::WindowAppearance;
    cx.update(|cx| {
        gpui_kit::init(cx);
        Mode::System.apply(cx);
        Mode::system_changed(WindowAppearance::Dark, cx);
        assert_eq!(cx.theme().appearance, Appearance::Dark, "System follows the system to dark");
        Mode::system_changed(WindowAppearance::Light, cx);
        assert_eq!(cx.theme().appearance, Appearance::Light, "and back to light");
        Mode::Dark.apply(cx);
        Mode::system_changed(WindowAppearance::Light, cx);
        assert_eq!(cx.theme().appearance, Appearance::Dark, "Dark stays dark");
    });
}

/// Agents lists each agent's models for the reader to sort and star: a press on a star makes that model the one new sessions start on, and
/// the list drawn follows the new default.
#[gpui_kit::test]
fn a_star_in_agents_makes_a_model_the_default(cx: &mut TestAppContext) {
    use atelier_settings::AgentModels;
    cx.update(|cx| {
        let mut all = crate::agent_models::ModelPrefs::default();
        all.0.insert(
            "claude-code".into(),
            AgentModels { known: vec![("claude-opus-5-5".into(), "Opus 5.5".into()), ("claude-sonnet-5-5".into(), "Sonnet 5.5".into())], ..AgentModels::default() },
        );
        cx.set_global(all);
    });
    let (_pane, cx, _) = open(&atelier_settings::Settings::default(), cx);
    click(cx, "section-agents");
    assert!(cx.debug_bounds("model-row-claude-code-claude-sonnet-5-5").is_some(), "the models of Claude Code are listed by their names");
    let start = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| crate::agent_models::start_model(&atelier_agents::registry::by_backend("claude-code").unwrap(), cx))
    };
    assert_eq!(start(cx).as_deref(), Some("claude-opus-5-5"), "the first, until one is starred");
    click(cx, "model-star-claude-code-claude-sonnet-5-5");
    assert_eq!(start(cx).as_deref(), Some("claude-sonnet-5-5"), "the starred model is the one new sessions start on");
}
