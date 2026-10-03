use atelier_agents::session::{Limit, LimitState, LimitWindow};
use atelier_ui::{
    menu::{self, Entry, Menu, MenuLook, Origin, Pick, entries_of},
    popover::{Hang, Popover},
    button::{Button, ButtonVariant},
    icon::{Icon, IconName},
    scale::px,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div};

use crate::agent_session::{AgentSession, SessionEvent, now};
use super::types::{DAY, HOUR, MINUTE, HANDOFF, MENU_GAP};

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

/// Whether the account is still at `limit` at `now`: past its reset it can work again, though `claude`
/// tells so only with the next message.
pub fn still_reached(limit: &Limit, now: u64) -> bool {
    limit.state == LimitState::Reached && limit.resets_at.is_none_or(|at| now < at)
}

/// The button that opens the handoff menu over the notice: the agents, and for an agent with a choice its providers.
/// A choice hands the session off at once.
fn handoff_button(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let s = session.read(cx);
    let (key, branches) = (s.key.clone(), s.handoff_branches.clone());
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-handoff-open").into()), cx, |_, _| false);
    let is_open = *open.read(cx) && !branches.is_empty();
    let shut = {
        let open = open.clone();
        move |cx: &mut App| open.update(cx, |o, cx| {
            *o = false;
            cx.notify();
        })
    };
    let menu = is_open.then(|| {
        let (owner, closing) = (session.clone(), shut.clone());
        // A choice shuts the menu, then asks for the handoff.
        let pick: Pick = std::rc::Rc::new(move |target, _, cx| {
            closing(cx);
            owner.update(cx, |_, cx| cx.emit(SessionEvent::Handoff(target.clone())));
        });
        let entries: Vec<Entry> = entries_of(&branches, &pick);
        let rows = entries.len();
        let close = shut.clone();
        Popover::new(gpui_kit::ElementId::Name(format!("{key}-handoff-popover").into()))
            .open(true)
            .hang(Hang::Right(0., -(menu::height_in(MenuLook::PROJECT, rows) + MENU_GAP)))
            .keep_focus()
            .height(menu::height_in(MenuLook::PROJECT, rows))
            .on_close(move |_, cx| close(cx))
            .child(Menu::new(gpui_kit::ElementId::Name(format!("{key}-handoff-menu").into()), entries).look(MenuLook::PROJECT).origin(Origin::BottomRight))
    });
    let toggle = open.clone();
    div()
        .relative()
        .child(
            Button::new(gpui_kit::ElementId::Name(format!("{key}-limit-handoff").into()))
                .label(HANDOFF)
                .variant(ButtonVariant::Secondary)
                .debug_name("limit-handoff")
                .open(is_open)
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |o, cx| {
                        *o = !*o;
                        cx.notify();
                    })
                }),
        )
        .children(menu)
}

/// The box over the composer while the session's account is at its limit, or `None`.
pub fn limit_notice(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let s = session.read(cx);
    let now = now();
    let limit = s.conversation.limit().filter(|limit| still_reached(limit, now))?;
    let words = limit_words(s.agent.name, &limit, now);
    let go_on = handoff_button(session, window, cx);
    let theme = cx.theme();
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
            .child(div().flex_1().min_w_0().whitespace_normal().child(words))
            .child(go_on)
            .into_any_element(),
    )
}
