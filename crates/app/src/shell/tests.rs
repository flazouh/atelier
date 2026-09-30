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
