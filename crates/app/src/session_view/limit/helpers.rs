use atelier_agents::session::{Limit, LimitState, LimitWindow};
use atelier_ui::{
    button::{Button, ButtonVariant},
    icon::{Icon, IconName},
    scale::px,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div};

use crate::agent_session::{AgentSession, SessionEvent, now};
use super::types::{CONTINUE_WITH, DAY, HOUR, MINUTE};

/// How long until `resets_at`, both in seconds since the Unix epoch: "in 2 h 14 min", "in 3 days".
pub fn resets_in(resets_at: u64, now: u64) -> String {
    let left = resets_at.saturating_sub(now);
    match left {
        0..MINUTE => "in a moment".into(),
        MINUTE..HOUR => format!("in {} min", left / MINUTE),
        HOUR..DAY => match (left % HOUR) / MINUTE {
            0 => format!("in {} h", left / HOUR),
            minutes => format!("in {} h {minutes} min", left / HOUR),
        },
        _ => match left / DAY {
            1 => "in a day".into(),
            days => format!("in {days} days"),
        },
    }
}

fn window_words(window: Option<LimitWindow>) -> &'static str {
    match window {
        Some(LimitWindow::FiveHour) => "5-hour limit",
        Some(LimitWindow::Weekly) => "weekly limit",
        Some(LimitWindow::Overage) => "extra usage limit",
        None => "usage limit",
    }
}

/// "Claude Code reached its 5-hour limit. It resets in 2 h 14 min."
pub fn limit_words(agent: &str, limit: &Limit, now: u64) -> String {
    let reached = format!("{agent} reached its {}.", window_words(limit.window));
    match limit.resets_at {
        Some(at) => format!("{reached} It resets {}.", resets_in(at, now)),
        None => reached,
    }
}

/// The box over the composer while the session's account is at its limit, or `None`.
pub fn limit_notice(session: &Entity<AgentSession>, cx: &App) -> Option<AnyElement> {
    let s = session.read(cx);
    let limit = s.conversation.limit().filter(|limit| limit.state == LimitState::Reached)?;
    let theme = cx.theme();
    let owner = session.clone();
    let go_on = Button::new(gpui_kit::ElementId::Name(format!("{}-limit-continue", s.key).into()))
        .label(CONTINUE_WITH)
        .variant(ButtonVariant::Secondary)
        .debug_name("limit-continue")
        .on_click(move |_, _, cx| owner.update(cx, |_, cx| cx.emit(SessionEvent::ContinueWith)));
    Some(
        div()
            .debug_selector(|| "limit-notice".into())
            .mx(px(12.))
            .mb(px(8.))
            .px(px(12.))
            .py(px(8.))
            .rounded(radius::lg())
            .bg(theme.card_strong)
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(TextSize::Xs.font_size())
            .child(Icon::new(IconName::Schedule).size(px(14.)).color(theme.warning))
            .child(div().flex_1().min_w_0().whitespace_normal().child(limit_words(s.agent.name, &limit, now())))
            .child(go_on)
            .into_any_element(),
    )
}
