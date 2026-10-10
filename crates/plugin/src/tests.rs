use std::{cell::RefCell, rc::Rc};

use atelier_ui::{IconName, ProviderGauge, menu::Lead};
use gpui_kit::{AnyElement, App, Context, IntoElement, TestAppContext, div};

use super::{AppHost, Host, Page, Plugin, PluginPage, PluginView, Registry, Vitals};

/// A page that counts what the app asks of it, and keeps its host to ask the app in turn.
struct Counter {
    host: Host,
    sidebars: usize,
    mains: usize,
    fronts: usize,
}

impl PluginPage for Counter {
    fn sidebar(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        self.sidebars += 1;
        div().into_any_element()
    }

    fn main(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        self.mains += 1;
        div().into_any_element()
    }

    fn in_front_again(&mut self, _cx: &mut Context<Self>) {
        self.fronts += 1;
    }
}

/// Another type of page, with nothing of its own but the two parts.
struct Plain;

impl PluginPage for Plain {
    fn sidebar(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        div().into_any_element()
    }

    fn main(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        div().into_any_element()
    }
}

/// Stands for the app: it keeps the ids it was asked to open, and has one provider on its bar.
#[derive(Clone, Default)]
struct FakeApp {
    opened: Rc<RefCell<Vec<String>>>,
}

impl AppHost for FakeApp {
    fn open_view(&self, id: &str, _cx: &mut App) {
        self.opened.borrow_mut().push(id.to_string());
    }

    fn vitals(&self, _cx: &App) -> Vitals {
        Vitals { load: None, providers: vec![ProviderGauge::new("Claude", Lead::Monogram)] }
    }
}

fn view(id: &'static str, order: i32) -> PluginView {
    PluginView::new(id, IconName::Code, id, order, |_, _| Plain)
}

fn ids(registry: &Registry) -> Vec<&'static str> {
    registry.views().iter().map(|view| view.id).collect()
}

struct Two;

impl Plugin for Two {
    fn register(&self, registry: &mut Registry) {
        registry.add_view(view("late", 20));
        registry.add_view(view("early", 10));
    }
}

#[test]
fn views_stand_by_order_then_by_id_and_a_view_with_the_id_of_another_replaces_it() {
    let mut registry = Registry::default();
    registry.add_view(view("b", 2));
    registry.add_view(view("c", 2));
    registry.add_view(view("a", 3));
    assert_eq!(ids(&registry), ["b", "c", "a"]);
    registry.add_view(view("b", 5));
    assert_eq!(ids(&registry), ["c", "a", "b"]);
    assert_eq!(registry.view("b").map(|view| view.order), Some(5));
    assert!(registry.view("nobody").is_none());
}

#[test]
fn a_plugin_registers_what_it_adds_and_without_it_the_registry_is_empty() {
    let mut registry = Registry::default();
    assert!(ids(&registry).is_empty());
    registry.add(&Two);
    assert_eq!(ids(&registry), ["early", "late"]);
}

#[gpui_kit::test]
fn the_app_reaches_the_page_a_view_makes_and_the_page_reaches_the_app_through_the_host(cx: &mut TestAppContext) {
    let app = FakeApp::default();
    let host = Host::new(app.clone());
    let view = PluginView::new("counter", IconName::Code, "Counter", 0, |host, _| Counter {
        host: host.clone(),
        sidebars: 0,
        mains: 0,
        fronts: 0,
    });
    let page: Page = cx.update(|cx| view.open(&host, cx));
    cx.update(|cx| {
        drop(page.sidebar(cx));
        drop(page.main(cx));
        drop(page.main(cx));
        page.in_front_again(cx);
    });
    let counter = page.downcast::<Counter>().expect("the page is the type its view made");
    assert_eq!(counter.read_with(cx, |c, _| (c.sidebars, c.mains, c.fronts)), (1, 2, 1));
    assert!(page.downcast::<Plain>().is_none(), "another type is refused");
    let providers = counter.update(cx, |c, cx| {
        c.host.open_view("other", cx);
        c.host.vitals(cx).providers.len()
    });
    assert_eq!(*app.opened.borrow(), ["other"], "the page asked the app through its host");
    assert_eq!(providers, 1, "and read the numbers of its bar");
}

#[gpui_kit::test]
fn a_page_need_not_listen_for_its_view_coming_back(cx: &mut TestAppContext) {
    let host = Host::new(FakeApp::default());
    let page = cx.update(|cx| view("plain", 0).open(&host, cx));
    cx.update(|cx| page.in_front_again(cx));
    assert!(page.downcast::<Plain>().is_some());
}
