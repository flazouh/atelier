//! The box over the composer for an agent that has no sign-in, in each state it goes through: not signed in, the
//! browser open, a sign-in that did not finish, and a project on another host. The handoff beside the button is the
//! way on that does not need the sign-in.
use atelier_agents::coding_agents::CodingAgent;
use atelier_ui::{
    Button, ButtonVariant, SignInNotice, SignInState,
    menu::Lead,
};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div, px};

use super::section;

fn handoff(id: &'static str) -> Button {
    Button::new(id).label("Handoff").variant(ButtonVariant::Secondary)
}

fn notice(id: &'static str, agent: CodingAgent, account: Option<&'static str>, state: SignInState, handoff_too: bool) -> impl IntoElement {
    let notice = SignInNotice::new(id, agent.name(), Lead::of(agent.mark())).account(account).state(state).on_sign_in(|_, _| {}).on_cancel(|_, _| {});
    div().w(px(560.)).child(if handoff_too { notice.action(handoff(id)).into_any_element() } else { notice.into_any_element() })
}

pub fn sign_in_story(_cx: &App) -> impl IntoElement {
    div()
        .child(section(
            "Not signed in: one button signs in, and the handoff goes on without",
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(notice("s-ready", CodingAgent::ClaudeCode, None, SignInState::Ready, true))
                .child(notice("s-account", CodingAgent::ClaudeCode, Some("work"), SignInState::Ready, true))
                .child(notice("s-cursor", CodingAgent::Cursor, None, SignInState::Ready, false)),
        ))
        .child(section(
            "The browser is open: the button waits, and Cancel leaves the wait",
            notice("s-waiting", CodingAgent::ClaudeCode, None, SignInState::Waiting, true),
        ))
        .child(section(
            "The sign-in did not finish: the reason, and the button tries again",
            notice("s-failed", CodingAgent::ClaudeCode, None, SignInState::Failed("The sign-in did not finish: it exited with code 1: browser closed".into()), true),
        ))
        .child(section(
            "A project on another host: a browser here cannot sign it in, so the notice says where",
            notice("s-elsewhere", CodingAgent::ClaudeCode, None, SignInState::Elsewhere("dev-box".into()), true),
        ))
}
