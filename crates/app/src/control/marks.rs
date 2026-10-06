//! Named places on the screen, for the control socket: an element drawn through [`marked`] says where it was
//! last drawn, so a script finds and presses it by name instead of by a guessed pixel. A debug build always marks;
//! a release build marks once the control socket listens, and draws the element as it is before that.

use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};

use gpui_kit::{AnyElement, App, Bounds, Global, IntoElement, ParentElement, Pixels, Styled, canvas, div};

static LISTENING: AtomicBool = AtomicBool::new(false);

/// Starts the marks in a release build, which the control socket does when it listens.
pub fn enable() {
    LISTENING.store(true, Ordering::Relaxed);
}

#[derive(Default)]
struct Marks(HashMap<&'static str, Bounds<Pixels>>);

impl Global for Marks {}

/// `element`, reporting its bounds as `name` each time it is drawn.
pub fn marked(name: &'static str, element: impl IntoElement) -> AnyElement {
    if !cfg!(debug_assertions) && !LISTENING.load(Ordering::Relaxed) {
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

/// Every name drawn so far, sorted.
pub fn names(cx: &App) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = cx.try_global::<Marks>().map(|marks| marks.0.keys().copied().collect()).unwrap_or_default();
    names.sort_unstable();
    names
}
