use std::sync::Arc;

use atelier_agents::usage_history::read_cached;
use atelier_ui::{Selection, UsageDashboard, UsageRange, modal::Modal, scale::px};
use gpui_kit::{
    Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::super::{
    consts::{LONGEST, MARGIN, MARGIN_SIDE},
    helpers::{build, today},
};
use super::{UsageState, UsageStore};
use crate::slots::Host;

/// The usage dashboard as a panel over the window, with what the reader chose in it. The logs are read off the UI thread,
/// and what was read stays in the [`UsageStore`] for the next opening, so the dashboard shows at once and then refreshes.
pub struct UsagePage {
    state: UsageState,
    focus: FocusHandle,
    host: Host,
}

impl UsagePage {
    /// Opens the dashboard with what was read last time, and reads the logs again.
    pub fn new(host: Host, cx: &mut Context<Self>) -> Self {
        let seen = cx.default_global::<UsageStore>().seen.clone();
        let page = Self {
            state: UsageState::new(seen),
            focus: cx.focus_handle(),
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

impl Render for UsagePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let readings = self.host.vitals.read(cx).providers().to_vec();
        let (today, _) = today();
        let view = build(&self.state, &readings, today);
        let (range, select, expand, host) = (
            cx.weak_entity(),
            cx.weak_entity(),
            cx.weak_entity(),
            self.host.clone(),
        );
        let dashboard = UsageDashboard::new("usage-dashboard")
            .range(self.state.range)
            .selection(self.state.selection.clone())
            .sources(view.sources)
            .summary(view.summary.0, view.summary.1)
            .days(view.days)
            .models(view.models)
            .sessions(view.sessions)
            .expanded(self.state.expanded.clone())
            .total(view.total)
            .empty(view.empty)
            .on_range(move |r, _, cx| drop(range.update(cx, |page, cx| page.set_range(r, cx))))
            .on_select(move |s, _, cx| drop(select.update(cx, |page, cx| page.select(s, cx))))
            .on_expand(move |id, _, cx| drop(expand.update(cx, |page, cx| page.expand(id, cx))));
        Modal::new("usage")
            .width((atelier_ui::scale::design(window.viewport_size().width) - 2. * MARGIN_SIDE).clamp(320., 1120.))
            .flush()
            .focus(&self.focus)
            .on_close(move |_, cx| host.close_view(cx))
            .child(
                // The window is the limit: a dashboard taller than it scrolls inside the panel.
                div()
                    .id("usage-scroll")
                    .track_focus(&self.focus)
                    .max_h(window.viewport_size().height - px(MARGIN))
                    .overflow_y_scroll()
                    .child(dashboard),
            )
    }
}
