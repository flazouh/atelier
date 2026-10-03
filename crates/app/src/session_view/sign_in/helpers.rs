use atelier_ui::{SignInNotice, menu::Lead};
use gpui_kit::{AnyElement, App, Entity, IntoElement, Window};

use crate::{agent_session::AgentSession, session_view::handoff_button::handoff_button};

/// The box over the composer while the session's agent has no sign-in, or `None`.
pub fn sign_in_notice(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let s = session.read(cx);
    let state = s.sign_in_state()?;
    let (key, name, mark, account) = (s.key.clone(), s.agent.name, s.agent.mark.clone(), s.sign_in_account());
    let can_hand_off = !s.handoff_branches.is_empty();
    let (owner, canceller) = (session.clone(), session.clone());
    let notice = SignInNotice::new(gpui_kit::ElementId::Name(format!("{key}-sign-in").into()), name, Lead::of(mark))
        .account(account)
        .state(state)
        .on_sign_in(move |_, cx| owner.update(cx, |s, cx| s.sign_in(cx)))
        .on_cancel(move |_, cx| canceller.update(cx, |s, cx| s.cancel_sign_in(cx)));
    Some(if can_hand_off { notice.action(handoff_button(session, "sign-in-handoff", window, cx)).into_any_element() } else { notice.into_any_element() })
}
