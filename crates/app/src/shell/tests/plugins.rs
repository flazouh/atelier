//! The views plugins register: an entry on the rail for each, the sidebar and the main area its page fills, the host
//! the page reaches the app through, and the settings that keep the view in front by its id.
use std::{cell::Cell, rc::Rc};

use atelier_plugin::{Host, Plugin, PluginPage, PluginView, Registry};
use atelier_ui::{IconName, scale::px};
use gpui_kit::{AnyElement, Context, Entity, TestAppContext, VisualTestContext, div, prelude::*};

use super::super::{Shell, ShellView};
use super::{open_shell_with, settle, with_a_session};
use crate::slots::Slots;

/// What the fake page tells the test: how often its view came back in front, and how many providers the host named
/// when its main area was last drawn.
#[derive(Default)]
struct Seen {
    returns: Cell<usize>,
    providers: Cell<usize>,
}

/// A tiny plugin: one view named fake, between Bots and Usage on the rail.
struct Fake(Rc<Seen>);

struct FakePage {
    host: Host,
    seen: Rc<Seen>,
}

impl PluginPage for FakePage {
    fn sidebar(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        div().debug_selector(|| "fake-sidebar".into()).size_full().child("Fake rows").into_any_element()
    }

    fn main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.seen.providers.set(self.host.vitals(cx).providers.len());
        let host = self.host.clone();
        let door = div()
            .id("fake-open-bots")
            .debug_selector(|| "fake-open-bots".into())
            .w(px(120.))
            .h(px(24.))
            .child("Open Bots")
            .on_click(move |_, _, cx| host.open_view("bots", cx));
        div().debug_selector(|| "fake-main".into()).size_full().child(door).into_any_element()
    }

    fn in_front_again(&mut self, _cx: &mut Context<Self>) {
        self.seen.returns.set(self.seen.returns.get() + 1);
    }
}

impl Plugin for Fake {
    fn register(&self, registry: &mut Registry) {
        let seen = self.0.clone();
        registry.add_view(PluginView::new("fake", IconName::Code, "Fake", 95, move |host, _| FakePage {
            host: host.clone(),
            seen: seen.clone(),
        }));
    }
}

fn press(name: &'static str, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn"));
    cx.simulate_click(at.center(), gpui_kit::Modifiers::default());
    settle(shell, cx);
}

fn view_of(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> ShellView {
    shell.read_with(cx, |s, _| s.view)
}

/// A plugin is one line in the registry: with it the rail has its entry, a press shows its sidebar and its main area,
/// another entry leaves it, and with the line gone the rail has no entry and the view opens nothing.
#[gpui_kit::test]
fn a_registered_plugin_is_on_the_rail_and_a_press_shows_its_sidebar_and_main_and_without_it_the_rail_has_no_entry(cx: &mut TestAppContext) {
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    assert!(cx.debug_bounds("rail-fake").is_none(), "nobody registered it yet");
    let seen = Rc::new(Seen::default());
    cx.update(|_, cx| cx.global_mut::<Slots>().plug(&Fake(seen.clone())));
    settle(&shell, cx);

    let top = |name: &'static str, cx: &mut VisualTestContext| cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is on the rail")).top();
    let order = ["rail-sessions", "rail-tasks", "rail-git", "rail-bots", "rail-fake", "rail-usage"].map(|name| top(name, cx));
    assert!(order.is_sorted_by(|a, b| a < b), "the app's own three, then the registered views by their order: {order:?}");
    assert!(cx.debug_bounds("fake-sidebar").is_none() && cx.debug_bounds("fake-main").is_none(), "not in front yet");

    press("rail-fake", &shell, cx);
    assert_eq!(view_of(&shell, cx), ShellView::Plugin("fake"));
    let (side, main) = (cx.debug_bounds("fake-sidebar").expect("its sidebar is drawn"), cx.debug_bounds("fake-main").expect("its main area is drawn"));
    let rail = cx.debug_bounds("view-rail").expect("the rail is drawn");
    assert!(rail.right() <= side.left() && side.right() <= main.left(), "the rail, its sidebar, then its main area: {rail:?} {side:?} {main:?}");
    assert!(cx.debug_bounds("project-switcher").is_some_and(|switcher| switcher.bottom() <= side.top()), "the project switcher heads its sidebar");
    assert!(cx.debug_bounds("panel-close").is_none(), "the sessions gave way");
    assert_eq!(shell.read_with(cx, crate::control::state)["view"], "fake", "the control socket names it by its id");
    assert_eq!(seen.returns.get(), 0, "the first time the page is made, not told");

    press("rail-sessions", &shell, cx);
    assert_eq!(view_of(&shell, cx), ShellView::Sessions);
    assert!(cx.debug_bounds("fake-sidebar").is_none() && cx.debug_bounds("fake-main").is_none(), "another entry leaves it");
    assert!(cx.debug_bounds("panel-close").is_some(), "the sessions are back");

    press("rail-fake", &shell, cx);
    assert_eq!(seen.returns.get(), 1, "the page is told its view came in front again");
    press("rail-fake", &shell, cx);
    assert!(!shell.read_with(cx, |s, _| s.sidebar), "a second press hides the sidebar, as for the app's own views");
    assert_eq!((view_of(&shell, cx), seen.returns.get()), (ShellView::Plugin("fake"), 1), "and the view stays");

    // The line out of the registry: the app as it starts has no such plugin.
    cx.update(|_, cx| cx.set_global(crate::slots::builtin()));
    settle(&shell, cx);
    assert!(cx.debug_bounds("rail-fake").is_none(), "no registration, no entry");
    assert!(cx.debug_bounds("fake-main").is_none(), "and no view");
    assert_eq!(view_of(&shell, cx), ShellView::Sessions, "the view in front gave way to Sessions");
    assert!(cx.debug_bounds("rail-bots").is_some() && cx.debug_bounds("rail-usage").is_some(), "the others stay");
    shell.update(cx, |shell, cx| shell.open_view("fake", cx));
    settle(&shell, cx);
    assert_eq!(view_of(&shell, cx), ShellView::Sessions, "a view nobody registered opens nothing");
}

/// The page reaches the app only through its host: it reads the numbers of the status bar, and opens another view by id.
#[gpui_kit::test]
fn a_page_reads_the_vitals_and_opens_another_view_through_its_host(cx: &mut TestAppContext) {
    use atelier_agents::usage::{Reading, Window};
    let (shell, cx, _dir) = with_a_session(cx, 1400.);
    let seen = Rc::new(Seen::default());
    cx.update(|_, cx| cx.global_mut::<Slots>().plug(&Fake(seen.clone())));
    shell.update(cx, |shell, cx| {
        shell.vitals.update(cx, |vitals, cx| {
            let reading = Reading { windows: vec![Window { label: "7d".into(), used: 0.4, resets_in: Some(60) }], note: None };
            vitals.settle("Claude", atelier_ui::menu::Lead::Monogram, Ok(reading));
            cx.notify();
        })
    });
    settle(&shell, cx);
    press("rail-fake", &shell, cx);
    assert_eq!(seen.providers.get(), 1, "the host hands the page the provider the bar shows");
    press("fake-open-bots", &shell, cx);
    assert_eq!(view_of(&shell, cx), ShellView::Plugin("bots"), "a page opens another view by its id");
    assert!(cx.debug_bounds("bots-sidebar").is_some() && cx.debug_bounds("fake-main").is_none());
}

/// The settings keep a plugin's view by its id: a window that closed on it opens on it, with its page made.
#[gpui_kit::test]
fn a_window_that_closed_on_a_plugin_view_opens_on_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let saved = atelier_settings::Settings { view: Some("bots".into()), ..Default::default() };
    let (shell, cx) = open_shell_with(cx, saved);
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(900.)));
    assert_eq!(view_of(&shell, cx), ShellView::Plugin("bots"));
    assert_eq!(ShellView::Plugin("bots").words(), "bots", "the name the settings keep is the id");
    shell.update_in(cx, |s, window, cx| s.open_local(dir.path().to_path_buf(), window, cx));
    settle(&shell, cx);
    assert!(cx.debug_bounds("bots-sidebar").is_some() && cx.debug_bounds("bots-view").is_some(), "the view is drawn with no press");
}

/// A saved id that no plugin registered (a plugin taken out since) opens Sessions.
#[gpui_kit::test]
fn a_saved_view_that_nobody_registered_opens_sessions(cx: &mut TestAppContext) {
    let saved = atelier_settings::Settings { view: Some("gone".into()), ..Default::default() };
    let (shell, cx) = open_shell_with(cx, saved);
    assert_eq!(view_of(&shell, cx), ShellView::Sessions);
    let slots = cx.update(|_, cx| cx.global::<Slots>().clone());
    assert_eq!(ShellView::saved(Some("usage"), Some(&slots)), ShellView::Plugin("usage"));
    assert_eq!(ShellView::saved(Some("usage"), Some(&Slots::default())), ShellView::Sessions, "the usage plugin taken out");
    assert_eq!(ShellView::saved(Some("files"), Some(&slots)), ShellView::Files, "the app's own views keep their names");
    assert_eq!(ShellView::saved(None, Some(&slots)), ShellView::Sessions);
}
