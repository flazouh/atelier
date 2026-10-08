//! The usage dashboard as a panel over the window.
use atelier_ui::{UsageDashboard, modal::Modal};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement, div, InteractiveElement};
use super::structs::Shell;
use crate::usage_view;
impl Shell {
    pub(super) fn usage_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.usage.as_ref()?;
        let readings = self.vitals.read(cx).providers().to_vec();
        let (today, _) = usage_view::today();
        let view = usage_view::build(state, &readings, today);
        let (range, select, expand, close) =
            (cx.entity().downgrade(), cx.entity().downgrade(), cx.entity().downgrade(), cx.entity().downgrade());
        let dashboard = UsageDashboard::new("usage-dashboard")
            .range(state.range)
            .selection(state.selection.clone())
            .sources(view.sources)
            .summary(view.summary.0, view.summary.1)
            .days(view.days)
            .models(view.models)
            .sessions(view.sessions)
            .expanded(state.expanded.clone())
            .total(view.total)
            .empty(view.empty)
            .on_range(move |r, _, cx| drop(range.update(cx, |shell, cx| shell.set_usage_range(r, cx))))
            .on_select(move |s, _, cx| drop(select.update(cx, |shell, cx| shell.select_usage(s, cx))))
            .on_expand(move |id, _, cx| drop(expand.update(cx, |shell, cx| shell.expand_usage(id, cx))));
        Some(
            Modal::new("usage")
                .width(1120.)
                .flush()
                .focus(&self.usage_focus)
                .on_close(move |_, cx| drop(close.update(cx, |shell, cx| shell.close_usage(cx))))
                .child(div().track_focus(&self.usage_focus).child(dashboard))
                .into_any_element(),
        )
    }
}
