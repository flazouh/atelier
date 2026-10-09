use std::{collections::HashMap, sync::OnceLock};

use atelier_capabilities::card::{Card, from_json};

use super::types::{PREFIX, Shape, TOOLS};

/// The cards the app ships, one for each tool of the gateway, as JSON data in the card schema (`docs/capabilities/card-schema-v1.md`).
const SOURCES: [(&str, &str); 16] = [
    ("tasks_list", include_str!("cards/tasks_list.json")),
    ("tasks_get", include_str!("cards/tasks_get.json")),
    ("tasks_search", include_str!("cards/tasks_search.json")),
    ("tasks_create", include_str!("cards/tasks_create.json")),
    ("tasks_update", include_str!("cards/tasks_update.json")),
    ("tasks_comment", include_str!("cards/tasks_comment.json")),
    (
        "messaging_channels",
        include_str!("cards/messaging_channels.json"),
    ),
    (
        "messaging_history",
        include_str!("cards/messaging_history.json"),
    ),
    (
        "messaging_thread",
        include_str!("cards/messaging_thread.json"),
    ),
    (
        "messaging_search",
        include_str!("cards/messaging_search.json"),
    ),
    ("messaging_send", include_str!("cards/messaging_send.json")),
    ("mail_mailboxes", include_str!("cards/mail_mailboxes.json")),
    ("mail_search", include_str!("cards/mail_search.json")),
    ("mail_thread", include_str!("cards/mail_thread.json")),
    ("mail_get", include_str!("cards/mail_get.json")),
    (
        "mail_create_draft",
        include_str!("cards/mail_create_draft.json"),
    ),
];

/// The raw JSON of each card, for the test that checks every one.
#[cfg(test)]
pub(super) fn sources() -> &'static [(&'static str, &'static str)] {
    &SOURCES
}

fn all() -> &'static HashMap<&'static str, Card> {
    static CARDS: OnceLock<HashMap<&'static str, Card>> = OnceLock::new();
    CARDS.get_or_init(|| {
        SOURCES
            .iter()
            // A card that is not valid is left out: its call keeps the plain row. The tests fail when one is not valid.
            .filter_map(|(tool, json)| from_json(json).ok().map(|card| (*tool, card)))
            .collect()
    })
}

/// The card of a tool, by the tool's own name (`tasks_list`).
pub fn card(tool: &str) -> Option<&'static Card> {
    all().get(tool)
}

/// The gateway tool a call is, from the name the agent gave it: `mcp__atelier__tasks_list` is `tasks_list`. `None` for any
/// other tool, and for a gateway tool that has no card.
pub fn tool_of(name: &str) -> Option<&'static str> {
    let tool = name.strip_prefix(PREFIX)?;
    TOOLS.iter().copied().find(|known| *known == tool)
}

/// What a tool's result must hold for its card to draw it.
pub fn shape(tool: &str) -> Option<Shape> {
    Some(match tool {
        "tasks_list" | "tasks_search" | "messaging_channels" | "messaging_history"
        | "messaging_thread" | "messaging_search" | "mail_mailboxes" | "mail_search" => {
            Shape::Items
        }
        "tasks_get" | "tasks_create" | "tasks_update" => Shape::One("task"),
        "tasks_comment" => Shape::One("comment"),
        "messaging_send" | "mail_get" => Shape::One("message"),
        "mail_thread" => Shape::One("thread"),
        "mail_create_draft" => Shape::One("draft"),
        _ => return None,
    })
}

/// Whether `result` is the shape `tool` gives.
pub fn fits(tool: &str, result: &serde_json::Value) -> bool {
    match shape(tool) {
        Some(Shape::Items) => result
            .get("items")
            .and_then(|v| v.as_array())
            .is_some_and(|rows| rows.iter().all(|r| r.is_object())),
        Some(Shape::One(key)) => result.get(key).is_some_and(|v| v.is_object()),
        None => false,
    }
}

/// The words for what a list holds, such as `task` and `tasks`.
pub fn nouns(tool: &str) -> (&'static str, &'static str) {
    match tool.split('_').next().unwrap_or_default() {
        "tasks" => ("task", "tasks"),
        "messaging" if tool == "messaging_channels" => ("channel", "channels"),
        "messaging" => ("message", "messages"),
        "mail" if tool == "mail_mailboxes" => ("mailbox", "mailboxes"),
        _ => ("thread", "threads"),
    }
}
