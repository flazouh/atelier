use atelier_capabilities::Ref;
use atelier_ui::ToolCardData;

/// A card ready to draw, and what each press on it asks for. The card gives back the number of the press as text.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub data: ToolCardData,
    pub presses: Vec<Press>,
}

/// What a row or a button of a card does. A card cannot make a call: it can only open what its result names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Press {
    Open(Ref),
}

/// The word an agent's tool calls begin with when they come through the gateway: Claude Code names a tool of an MCP server
/// `mcp__<server>__<tool>`, and the server is `atelier`.
pub const PREFIX: &str = "mcp__atelier__";

/// The tools of the gateway that have a card, and the words that count what they found.
pub const TOOLS: [&str; 16] = [
    "tasks_list",
    "tasks_get",
    "tasks_search",
    "tasks_create",
    "tasks_update",
    "tasks_comment",
    "messaging_channels",
    "messaging_history",
    "messaging_thread",
    "messaging_search",
    "messaging_send",
    "mail_mailboxes",
    "mail_search",
    "mail_thread",
    "mail_get",
    "mail_create_draft",
];

/// Where the data of a result lies. A result that does not have it is not the shape the card expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `items` is an array of objects.
    Items,
    /// The key holds one object.
    One(&'static str),
}
