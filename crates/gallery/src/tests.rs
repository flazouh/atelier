use gpui_kit::{TestAppContext, px, size};

use super::*;

/// The sidebar lists more stories than fit in an 1100 by 860 window: the list scrolls, and the theme
/// picker at its foot stays whole in the window.
#[gpui_kit::test]
fn the_theme_picker_stays_in_a_short_window(cx: &mut TestAppContext) {
    cx.update(|cx| {
        beui::init(cx);
        beui::theme::set_theme(beui::themes::lathe(Appearance::Dark).clone(), cx);
    });
    let (_gallery, cx) = cx.add_window_view(Gallery::new);
    cx.simulate_resize(size(px(1100.), px(860.)));
    cx.run_until_parked();
    let picker = cx.debug_bounds("theme-picker").expect("the picker is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert!(picker.bottom() <= window.height, "the picker ends at {:?}, in a window {:?} tall", picker.bottom(), window.height);
    assert!(picker.top() >= px(0.));
}
