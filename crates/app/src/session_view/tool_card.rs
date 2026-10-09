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
        .map(|hub| agent_of(&hub.person().name))
}

/// "Alex's agent", as the Messages screen words it: the name starts with a capital.
fn agent_of(owner: &str) -> String {
    let mut letters = owner.chars();
    let name: String = letters
        .next()
        .map(|c| c.to_uppercase().chain(letters).collect())
        .unwrap_or_default();
    format!("{name}'s agent")
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

#[cfg(test)]
mod tests {
    use super::agent_of;

    #[test]
    fn the_origin_names_the_owner_with_a_capital() {
        assert_eq!(agent_of("alex"), "Alex's agent");
        assert_eq!(agent_of("Sam"), "Sam's agent");
    }
}
