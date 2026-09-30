use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, size, px};

use super::*;

fn open<'a>(hosts: &[&str], cx: &'a mut TestAppContext) -> (Entity<SshForm>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let hosts: Vec<String> = hosts.iter().map(|h| h.to_string()).collect();
    let (form, cx) = cx.add_window_view(move |window, cx| SshForm::new(hosts, window, cx));
    cx.simulate_resize(size(px(438.), px(700.))); // the width the modal leaves the form
    for _ in 0..3 {
        cx.run_until_parked();
        form.update(cx, |_, cx| cx.notify());
    }
    cx.run_until_parked();
    (form, cx)
}

#[gpui_kit::test]
fn the_hosts_wrap_under_the_first_chip_and_not_under_the_label(cx: &mut TestAppContext) {
    let (_, cx) = open(&["phone", "phone-usb", "phone-termux", "air", "pro", "hp-agent"], cx);
    let chips: Vec<_> = (0..6).map(|i| cx.debug_bounds(HOST_CHIPS[i]).unwrap_or_else(|| panic!("chip {i} is not drawn"))).collect();
    let first = chips[0];
    let wrapped: Vec<_> = chips.iter().filter(|c| c.top() > first.top() + px(1.)).collect();
    assert!(!wrapped.is_empty(), "six hosts do not fit on one line of the form");
    assert_eq!(wrapped[0].left(), first.left(), "the second line starts under the first chip");
    let row = cx.debug_bounds("ssh-hosts").expect("the chips box is drawn");
    assert!(row.left() > px(0.) && first.left() == row.left(), "the chips sit after the label");
}

#[gpui_kit::test]
fn a_host_chip_is_a_filled_button_and_a_press_puts_its_name_in_the_field(cx: &mut TestAppContext) {
    let (form, cx) = open(&["phone", "hp-agent"], cx);
    let at = cx.debug_bounds("ssh-host-1").expect("drawn").center();
    cx.simulate_click(at, Modifiers::default());
    cx.run_until_parked();
    let value = form.read_with(cx, |f, cx| f.host.read(cx).value().to_string());
    assert_eq!(value, "hp-agent");
}
