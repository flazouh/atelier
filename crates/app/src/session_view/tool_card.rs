//! The card of a gateway tool call (`crate::tool_card`) as the session draws it: in place of the plain row of a call that has
//! one, and above the approval of a call that waits for the reader.
use atelier_agents::session::{ToolCall, ToolOutput};
use atelier_ui::ToolCard;
use gpui_kit::{AnyElement, App, ElementId, Entity, IntoElement, Window};

use crate::{
    agent_session::{AgentSession, SessionEvent},
    capability_hub::CapabilityHub,
    tool_card::{Press, card_for, now_ms},
};

/// Whose agent made the call: the person the app acts for, as "Alex's agent".
fn origin(cx: &App) -> Option<String> {
    cx.try_global::<CapabilityHub>()
        .map(|hub| format!("{}'s agent", hub.person().name))
}

/// The card for `call`, or `None` when it keeps the plain row. A press on a row or a button asks the session to show what it
/// names; the card never calls anything.
pub(super) fn card(
    session: &Entity<AgentSession>,
    id: impl Into<ElementId>,
    call: &ToolCall,
    output: Option<&ToolOutput>,
    waiting: bool,
    cx: &App,
) -> Option<AnyElement> {
    let built = card_for(call, output, waiting, origin(cx), now_ms())?;
    let (opener, presses) = (session.downgrade(), built.presses);
    Some(
        ToolCard::new(id, built.data)
            .on_action(move |key: &str, _: &mut Window, cx: &mut App| {
                let Some(Press::Open(reference)) =
                    key.parse::<usize>().ok().and_then(|n| presses.get(n))
                else {
                    return;
                };
                let reference = reference.clone();
                drop(opener.update(cx, |_, cx| cx.emit(SessionEvent::OpenRef(reference))));
            })
            .into_any_element(),
    )
}
