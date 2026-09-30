//! Views drawn from their last frame until they change (`plans/view-cache.md`). A cached view is laid out
//! and painted again only when its entity is notified, so each one must be told when what it shows changes.
//! `LATHE_VIEW_CACHE=0` draws every view afresh, to prove or rule out a stale view in one run.

use std::sync::OnceLock;

use gpui_kit::{AnyElement, Entity, IntoElement, Render, StyleRefinement, Styled};

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("LATHE_VIEW_CACHE").map_or(true, |v| v != "0"))
}

/// `view`, filling its box, from its last frame when nothing it shows has changed.
pub fn draw<V: Render>(view: &Entity<V>) -> AnyElement {
    if enabled() {
        view.clone().cached(StyleRefinement::default().size_full()).into_any_element()
    } else {
        view.clone().into_any_element()
    }
}
