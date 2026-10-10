use std::sync::Arc;

use atelier_agents::usage_history::read_cached;
use atelier_ui::{Selection, UsageDashboard, UsageRange, UsageSources, scale::px, typography::TextSize};
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::super::{
    consts::{DETAIL_PAD, LONGEST},
    helpers::{build, today},
};
use super::{UsageState, UsageStore};
use crate::slots::Host;

/// The usage dashboard as a panel over the window, with what the reader chose in it. The logs are read off the UI thread,
/// and what was read stays in the [`UsageStore`] for the next opening, so the dashboard shows at once and then refreshes.
pub struct UsagePage {
    state: UsageState,
    host: Host,
}

impl UsagePage {
    /// Opens the dashboard with what was read last time, and reads the logs again.
    pub fn new(host: Host, cx: &mut Context<Self>) -> Self {
        let seen = cx.default_global::<UsageStore>().seen.clone();
        let page = Self {
            state: UsageState::new(seen),
            host,
        };
        page.read(cx);
        page
    }

    fn set_range(&mut self, range: UsageRange, cx: &mut Context<Self>) {
        self.state.range = range;
        self.state.expanded = None;
        cx.notify();
    }

    fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.state.selection = selection;
        self.state.expanded = None;
        cx.notify();
    }

    fn expand(&mut self, session: Option<SharedString>, cx: &mut Context<Self>) {
        self.state.expanded = session;
        cx.notify();
    }

    /// Reads the session logs of the last month in the background; a file that did not change is not read again.
    fn read(&self, cx: &mut Context<Self>) {
        let store = cx.default_global::<UsageStore>();
        let (cache, roots) = (store.cache.clone(), store.roots.clone());
        let (today, offset) = today();
        cx.spawn(async move |this, cx| {
            let history = cx
                .background_executor()
                .spawn(async move {
                    let since = today.minus_days(i64::from(LONGEST));
                    let mut cache = cache
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    read_cached(&roots, since, offset, &mut cache)
                })
                .await;
            let history = Arc::new(history);
            // Kept even when the dashboard was closed meanwhile: the next opening starts from it.
            let kept = history.clone();
            cx.update(|cx| cx.default_global::<UsageStore>().seen = kept);
            drop(this.update(cx, |page, cx| {
                page.state.history = history;
                page.state.loading = false;
                cx.notify();
            }));
        })
        .detach();
    }
}

impl UsagePage {
    /// The view came in front again: reads the logs once more, and keeps what it shows until they are read.
    pub fn refresh(&self, cx: &mut Context<Self>) {
        self.read(cx);
    }

    /// What the view shows now, built again only when the state or the readings changed.
    fn built(&self, cx: &mut Context<Self>) -> crate::usage_view::structs::UsageView {
        let readings = self.host.vitals.read(cx).providers().to_vec();
        let (today, _) = today();
        build(&self.state, &readings, today)
    }

    /// The sidebar of the Usage view: its title and the list of sources, as the other views' sidebars are filled.
    pub fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = self.built(cx);
        let select = cx.weak_entity();
        let sources = UsageSources::new("usage-sources")
            .sources(view.sources)
            .summary(view.summary.0, view.summary.1)
            .selection(self.state.selection.clone())
            .on_select(move |s, _, cx| drop(select.update(cx, |page, cx| page.select(s, cx))));
        div()
            .id("usage-side")
            .debug_selector(|| "usage-sidebar".into())
            .size_full()
            .overflow_y_scroll()
            .px(px(4.))
            .pb(px(8.))
            .child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(8.))
                    .text_size(TextSize::Lg.font_size())
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Usage"),
            )
            .child(sources)
            .into_any_element()
    }

    /// The detail of what is chosen, scrolling inside the card the shell puts it on.
    pub fn main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = self.built(cx);
        let (range, expand) = (cx.weak_entity(), cx.weak_entity());
        let dashboard = UsageDashboard::new("usage-dashboard")
            .range(self.state.range)
            .title(view.title)
            .subtitle(view.subtitle)
            .stats(view.stats)
            .sources(view.sources)
            .days(view.days)
            .models(view.models)
            .sessions(view.sessions)
            .expanded(self.state.expanded.clone())
            .total(view.total)
            .empty(view.empty)
            .on_range(move |r, _, cx| drop(range.update(cx, |page, cx| page.set_range(r, cx))))
            .on_expand(move |id, _, cx| drop(expand.update(cx, |page, cx| page.expand(id, cx))));
        let dashboard = match (view.dot, view.provider) {
            (Some(dot), Some(provider)) => dashboard.dot(dot).provider(provider),
            (Some(dot), None) => dashboard.dot(dot),
            (None, Some(provider)) => dashboard.provider(provider),
            (None, None) => dashboard,
        };
        div()
            .id("usage-scroll")
            .debug_selector(|| "usage-view".into())
            .size_full()
            .overflow_y_scroll()
            .p(px(DETAIL_PAD))
            .child(dashboard)
            .into_any_element()
    }
}

/// Shown on its own (the slot's view), the page is its detail.
impl Render for UsagePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.main(cx)
    }
}
