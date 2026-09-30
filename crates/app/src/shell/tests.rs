use gpui_kit::{TestAppContext, px, size};

use super::*;

#[gpui_kit::test]
fn the_first_launch_shows_the_mark_and_one_line_about_what_lathe_is_above_the_buttons(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&lathe_settings::Settings::default(), cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    for _ in 0..3 {
        cx.run_until_parked();
        shell.update(cx, |_, cx| cx.notify());
    }
    let mark = cx.debug_bounds("lathe-mark").expect("the mark is drawn");
    let line = cx.debug_bounds("first-launch-line").expect("the line is drawn");
    let button = cx.debug_bounds("open-folder").or_else(|| cx.debug_bounds("open-folder-button"));
    assert_eq!((f32::from(mark.size.width), f32::from(mark.size.height)), (40., 40.));
    assert!(mark.bottom() <= line.top(), "the mark is above the line: {mark:?} {line:?}");
    if let Some(button) = button {
        assert!(line.bottom() <= button.top(), "the line is above the buttons");
    }
    assert!(WHAT_LATHE_IS.split_whitespace().count() <= 20, "one short line");
}
/// Quit has a key, Command-Q on the Mac and Control-Q elsewhere.
#[gpui_kit::test]
fn quit_has_its_key(cx: &mut TestAppContext) {
    cx.update(|cx| {
        bind_keys(cx);
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let bound: Vec<String> = keymap.bindings_for_action(&Quit).map(|b| b.keystrokes().iter().map(|k| k.unparse()).collect::<Vec<_>>().join(" ")).collect();
        let want = if cfg!(target_os = "macos") { "cmd-q" } else { "ctrl-q" };
        assert!(bound.iter().any(|b| b == want), "{bound:?}");
    });
}

fn open_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut gpui_kit::VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(&lathe_settings::Settings::default(), cx));
    cx.simulate_resize(size(px(1200.), px(800.)));
    (shell, cx)
}

fn settle(shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        shell.update(cx, |_, cx| cx.notify());
    }
}

/// Start screen: a long error shows in the screen above Recent and wraps inside its column.
#[gpui_kit::test]
fn a_long_error_shows_on_the_start_screen_above_recent_and_wraps(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    shell.update(cx, |s, cx| s.say("Could not open the folder: the path does not exist and this line is long enough to need more than one line of the start screen column".into(), cx));
    settle(&shell, cx);
    let error = cx.debug_bounds("start-error").expect("the error is on the start screen");
    let recent = cx.debug_bounds("recent-heading").expect("Recent is drawn");
    assert!(error.bottom() <= recent.top(), "the error is above Recent");
    assert!(error.size.width <= px(420.), "the error stays in the column: {error:?}");
    assert!(error.size.height > px(20.), "the error wraps to more than one line: {error:?}");
}

/// A view that draws only the sidebar foot.
struct Foot(Entity<Shell>);

impl Render for Foot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let foot = self.0.update(cx, |shell, cx| shell.sidebar_foot(cx));
        div().w(px(240.)).child(foot)
    }
}

/// A12: the sidebar foot holds the Settings entry, with its key on the cap, and no theme picker.
#[gpui_kit::test]
fn the_sidebar_foot_holds_settings_and_not_the_theme_picker(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let (foot, cx) = cx.add_window_view(|_, _| Foot(shell.clone()));
    cx.simulate_resize(size(px(300.), px(200.)));
    for _ in 0..3 {
        cx.run_until_parked();
        foot.update(cx, |_, cx| cx.notify());
    }
    assert!(cx.debug_bounds("settings-entry").is_some(), "the Settings entry stands in the foot");
    assert!(cx.debug_bounds("theme").is_none(), "no theme picker in the foot");
}

/// K1: every global chord reaches its action from each place the focus can be. Tasks is the chord that
/// was lost; each pane is given the focus in turn, and the chord must show Tasks from there.
#[gpui_kit::test]
fn the_tasks_chord_reaches_its_action_from_each_pane(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    let chord = if cfg!(target_os = "macos") { "cmd-shift-l" } else { "ctrl-shift-l" };
    let front = |shell: &Entity<Shell>, cx: &mut gpui_kit::VisualTestContext| {
        shell.read_with(cx, |s, cx| s.active().map(|p| p.read(cx).front()))
    };
    type Place = fn(&mut Shell, &mut Window, &mut Context<Shell>);
    let places: [(&str, Place); 4] = [
        ("nothing", |_, window, cx| window.blur(cx)),
        ("the shell", |s, window, cx| s.focus.focus(window, cx)),
        ("the sidebar", |s, window, cx| s.agents_sidebar.read(cx).focus_handle(cx).focus(window, cx)),
        ("a session's composer", |s, window, cx| s.new_session_key(&NewSession, window, cx)),
    ];
    let tasks = Some(crate::open_project::front::Front::Tasks);
    let mut lost = Vec::new();
    for (name, place) in places {
        shell.update_in(cx, |s, window, cx| place(s, window, cx));
        settle(&shell, cx);
        assert_ne!(front(&shell, cx), tasks, "Tasks starts hidden ({name})");
        cx.simulate_keystrokes(chord);
        settle(&shell, cx);
        if front(&shell, cx) != tasks {
            lost.push(format!("from {name}"));
            continue;
        }
        // From the Tasks pane itself the chord hides it again.
        cx.simulate_keystrokes(chord);
        settle(&shell, cx);
        if front(&shell, cx) == tasks {
            lost.push(format!("from the Tasks pane, after {name}"));
            shell.update_in(cx, |s, window, cx| s.show_tasks(window, cx));
        }
    }
    assert!(lost.is_empty(), "{chord} did not reach Tasks {lost:?}");
}

/// K1, for the whole table: every lathe chord bound with no context is on the dispatch path from each place
/// the focus can be, so no pane and no lost focus eats it.
#[gpui_kit::test]
fn every_global_chord_reaches_its_action_from_each_pane(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    let (shell, cx) = open_shell(cx);
    cx.update(|_, cx| bind_keys(cx));
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    let global: Vec<(String, Box<dyn gpui_kit::Action>)> = cx.update(|_, cx| {
        let map = cx.key_bindings();
        let map = map.borrow();
        map.bindings()
            .filter(|b| b.predicate().is_none() && b.action().name().starts_with("lathe::"))
            .map(|b| (b.keystrokes().iter().map(|k| k.inner().unparse()).collect::<Vec<_>>().join(" "), b.action().boxed_clone()))
            .collect()
    });
    assert!(global.len() >= 10, "the shell's chords are bound: {}", global.len());
    type Place = fn(&mut Shell, &mut Window, &mut Context<Shell>);
    let places: [(&str, Place); 5] = [
        ("nothing", |_, window, cx| window.blur(cx)),
        ("the shell", |s, window, cx| s.focus.focus(window, cx)),
        ("the sidebar", |s, window, cx| s.agents_sidebar.read(cx).focus_handle(cx).focus(window, cx)),
        ("a session's composer", |s, window, cx| s.new_session_key(&NewSession, window, cx)),
        ("the Tasks pane", |s, window, cx| s.show_tasks(window, cx)),
    ];
    let mut lost = Vec::new();
    for (name, place) in places {
        shell.update_in(cx, |s, window, cx| place(s, window, cx));
        settle(&shell, cx);
        let missing: Vec<String> = cx.update(|window, cx| {
            global.iter().filter(|(_, action)| !window.is_action_available(action.as_ref(), cx)).map(|(keys, _)| keys.clone()).collect()
        });
        if !missing.is_empty() {
            lost.push(format!("from {name}: {missing:?}"));
        }
    }
    assert!(lost.is_empty(), "chords that do not reach their action {lost:#?}");
}
