//! The usage dashboard as a panel over the window.
use atelier_ui::{UsageDashboard, modal::Modal};
use atelier_ui::scale::px;
use gpui_kit::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div};
use super::structs::Shell;
use crate::usage_view;
/// The room the panel leaves above and below it, in design pixels.
const MARGIN: f32 = 96.;
/// The room the panel leaves at each side in a narrow window.
const MARGIN_SIDE: f32 = 24.;
impl Shell {
    pub(super) fn usage_panel(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                .width((f32::from(window.viewport_size().width) - 2. * MARGIN_SIDE).clamp(320., 1120.))
                .flush()
                .focus(&self.usage_focus)
                .on_close(move |_, cx| drop(close.update(cx, |shell, cx| shell.close_usage(cx))))
                .child(
                    // The window is the limit: a dashboard taller than it scrolls inside the panel.
                    div()
                        .id("usage-scroll")
                        .track_focus(&self.usage_focus)
                        .max_h(window.viewport_size().height - px(MARGIN))
                        .overflow_y_scroll()
                        .child(dashboard),
                )
                .into_any_element(),
        )
    }
}
