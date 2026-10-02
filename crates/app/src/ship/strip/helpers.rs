use super::structs::CommitNow;

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([gpui_kit::KeyBinding::new("secondary-enter", CommitNow, Some("ShipComposer > Input"))]);
}

/// "1 file", or "N files".
pub(super) fn files(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}
