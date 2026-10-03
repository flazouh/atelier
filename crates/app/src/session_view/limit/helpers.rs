use atelier_agents::session::{Limit, LimitState, LimitWindow};
use atelier_ui::{
    button::{Button, ButtonVariant},
    icon::{Icon, IconName},
    scale::px,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div};

use crate::{agent_session::{AgentSession, SessionEvent, now}, providers::Choice};
use super::types::{AT_ITS_LIMIT, CONTINUE_WITH, DAY, HOUR, MENU_GAP, MINUTE, OTHER_AGENTS};

/// How long until `resets_at`, both in seconds since the Unix epoch, to the minute while it is a day off and the hour
/// beyond: "in 2 h 39 min", "in 1 d 6 h", "in 3 d".
pub fn resets_in(resets_at: u64, now: u64) -> String {
    let left = resets_at.saturating_sub(now);
    let parts = |big: u64, big_unit: &str, small: u64, small_unit: &str| match small {
        0 => format!("in {big} {big_unit}"),
        small => format!("in {big} {big_unit} {small} {small_unit}"),
    };
    match left {
        0..MINUTE => "in a moment".into(),
        MINUTE..HOUR => format!("in {} min", left / MINUTE),
        HOUR..DAY => parts(left / HOUR, "h", (left % HOUR) / MINUTE, "min"),
        _ => parts(left / DAY, "d", (left % DAY) / HOUR, "h"),
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

/// What a choice in the dropdown does: go on in a new session on this agent and provider.
struct Way {
    label: String,
    note: Option<&'static str>,
    backend: String,
    provider: Option<Choice>,
    disabled: bool,
}

/// The agent's own providers first, the account that is at its limit shown but not offered, then the other agents.
/// Only the agent in the session has its accounts read, so another agent with providers is offered on its default.
fn ways(s: &AgentSession) -> (Option<(String, Vec<Way>)>, Vec<Way>) {
    let agents = atelier_agents::registry::agents();
    let mine = s.agent.backend.name().to_string();
    let providers = s.provider.is_some().then(|| {
        let ways = s
            .provider_choices()
            .into_iter()
            .map(|choice| Way {
                label: crate::providers::label(&choice, &s.provider_accounts),
                note: (s.provider.as_ref() == Some(&choice)).then_some(AT_ITS_LIMIT),
                disabled: s.provider.as_ref() == Some(&choice),
                backend: mine.clone(),
                provider: Some(choice),
            })
            .collect();
        (s.agent.name.to_string(), ways)
    });
    let others = agents
        .iter()
        .filter(|a| providers.is_none() || a.backend.name() != mine)
        .map(|a| Way { label: a.name.to_string(), note: None, backend: a.backend.name().to_string(), provider: None, disabled: false })
        .collect();
    (providers, others)
}

/// The box over the composer while the session's account is at its limit, or `None`. Its button opens the agents and
/// providers to go on with, upward.
pub fn limit_notice(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    use atelier_ui::{
        menu::{self, Entry, Menu, MenuItem, MenuLook, Origin},
        popover::{Hang, Popover},
    };
    let key = session.read(cx).key.clone();
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-limit-menu").into()), cx, |_, _| false);
    let is_open = *open.read(cx);
    let s = session.read(cx);
    let limit = s.conversation.limit().filter(|limit| limit.state == LimitState::Reached)?;
    let words = limit_words(s.agent.name, &limit, now());
    let (providers, others) = ways(s);
    let menu = is_open.then(|| {
        // A choice shuts the menu, then asks for the new session.
        let way = |way: Way| {
            let (owner, open) = (session.clone(), open.clone());
            let (backend, provider) = (way.backend.clone(), way.provider.clone());
            let mut item = MenuItem::new(way.label.clone()).debug_name(format!("limit-continue-{}", way.label)).disabled(way.disabled).on_select(move |_, cx| {
                open.update(cx, |o, cx| {
                    *o = false;
                    cx.notify();
                });
                let (backend, provider) = (backend.clone(), provider.clone());
                owner.update(cx, |_, cx| cx.emit(SessionEvent::ContinueOn { backend, provider }));
            });
            if let Some(note) = way.note {
                item = item.description(note);
            }
            Entry::from(item)
        };
        let mut entries = Vec::new();
        if let Some((agent, ways)) = providers {
            entries.push(Entry::Label(agent.into()));
            entries.extend(ways.into_iter().map(way));
            entries.push(Entry::Separator);
            entries.push(Entry::Label(OTHER_AGENTS.into()));
        }
        entries.extend(others.into_iter().map(way));
        let height = menu::height_of(MenuLook::PROJECT, &entries);
        let close = open.clone();
        Popover::new(gpui_kit::ElementId::Name(format!("{key}-limit-popover").into()))
            .open(true)
            .hang(Hang::Right(0., -height - MENU_GAP))
            .keep_focus()
            .height(height)
            .on_close(move |_, cx| {
                close.update(cx, |o, cx| {
                    *o = false;
                    cx.notify();
                });
            })
            .child(Menu::new(gpui_kit::ElementId::Name(format!("{key}-limit-menu-panel").into()), entries).look(MenuLook::PROJECT).origin(Origin::BottomRight))
    });
    let theme = cx.theme();
    let go_on = Button::new(gpui_kit::ElementId::Name(format!("{key}-limit-continue").into()))
        .label(CONTINUE_WITH)
        .trailing_icon(IconName::ChevronDown)
        .variant(ButtonVariant::Secondary)
        .debug_name("limit-continue")
        .open(is_open)
        .on_click({
            let open = open.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                open.update(cx, |o, cx| {
                    *o = !*o;
                    cx.notify();
                })
            }
        });
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
            .child(div().relative().child(crate::control::marked("limit-continue", go_on)).children(menu))
            .into_any_element(),
    )
}
