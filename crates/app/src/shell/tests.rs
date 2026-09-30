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
