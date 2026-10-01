use super::structs::OpenNow;

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([gpui_kit::KeyBinding::new("secondary-enter", OpenNow, Some("PullForm > Input"))]);
}
