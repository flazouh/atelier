use std::sync::Arc;

use atelier_agents::usage_history::read_cached;
use atelier_ui::{
    Selection, UsageDashboard, UsageRange, UsageSources,
    scale::px,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::super::{
    consts::{DETAIL_PAD, LONGEST, SIDE_WIDTH},
    helpers::{build, today},
};
use super::{UsageState, UsageStore};
use crate::shell::TITLE_BAR;
use crate::slots::Host;

/// The usage dashboard as a panel over the window, with what the reader chose in it. The logs are read off the UI thread,
/// and what was read stays in the [`UsageStore`] for the next opening, so the dashboard shows at once and then refreshes.
pub struct UsagePage {
    state: UsageState,
    focus: FocusHandle,
    host: Host,
    /// Whether the page took the focus yet, so Escape reaches it.
    focused: bool,
}

impl UsagePage {
    /// Opens the dashboard with what was read last time, and reads the logs again.
    pub fn new(host: Host, cx: &mut Context<Self>) -> Self {
        let seen = cx.default_global::<UsageStore>().seen.clone();
        let page = Self {
            state: UsageState::new(seen),
            focus: cx.focus_handle(),
            host,
            focused: false,
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
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let host = self.host.clone();
        let readings = self.host.vitals.read(cx).providers().to_vec();
        let (today, _) = today();
        let view = build(&self.state, &readings, today);
        let theme = cx.theme().clone();
        let (range, select, expand) = (cx.weak_entity(), cx.weak_entity(), cx.weak_entity());
        let sources = UsageSources::new("usage-sources")
            .sources(view.sources.clone())
            .summary(view.summary.0.clone(), view.summary.1.clone())
            .selection(self.state.selection.clone())
            .on_select(move |s, _, cx| drop(select.update(cx, |page, cx| page.select(s, cx))));
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
        // The view fills the window right of the rail and between the title bar and the status bar: a list of the
        // sources at the left, the detail of the one chosen at the right, as the other views of the shell are laid out.
        div()
            .id("usage-view")
            .debug_selector(|| "usage-view".into())
            .absolute()
            .top(px(TITLE_BAR))
            .left(px(atelier_ui::view_rail::WIDTH))
            .right_0()
            .bottom(px(atelier_ui::status_bar::HEIGHT))
            .flex()
            .bg(theme.background)
            .track_focus(&self.focus)
            .on_key_down(move |event, _, cx| {
                if event.keystroke.key == "escape" {
                    host.close_view(cx);
                }
            })
            .child(
                div()
                    .id("usage-side")
                    .flex_none()
                    .w(px(SIDE_WIDTH))
                    .h_full()
                    .overflow_y_scroll()
                    .pl(px(8.))
                    .pr(px(4.))
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
                    .child(sources),
            )
            .child(
                div()
                    .id("usage-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .px(px(DETAIL_PAD))
                    .pt(px(10.))
                    .pb(px(DETAIL_PAD))
                    .rounded(radius::lg())
                    .child(dashboard),
            )
    }
}
