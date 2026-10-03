//! Named places on the screen, for the control socket: an element drawn through [`marked`] says where it was
//! last drawn, so a script finds and presses it by name instead of by a guessed pixel. Only a debug build marks;
//! a release build draws the element as it is.

use std::collections::HashMap;

use gpui_kit::{AnyElement, App, Bounds, Global, IntoElement, ParentElement, Pixels, Styled, canvas, div};

#[derive(Default)]
struct Marks(HashMap<&'static str, Bounds<Pixels>>);

impl Global for Marks {}

/// `element`, reporting its bounds as `name` each time it is drawn.
pub fn marked(name: &'static str, element: impl IntoElement) -> AnyElement {
    if !cfg!(debug_assertions) {
        return element.into_any_element();
    }
    div()
        .relative()
        .child(element)
        .child(canvas(move |bounds, _, cx| cx.default_global::<Marks>().0.insert(name, bounds), |_, _, _, _| {}).absolute().inset_0())
        .into_any_element()
}

/// Where `name` was last drawn, in the window's own points.
pub fn find(name: &str, cx: &App) -> Option<Bounds<Pixels>> {
    cx.try_global::<Marks>()?.0.get(name).copied()
}
