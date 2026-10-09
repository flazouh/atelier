//! The tool card: how the result of a tool call is drawn in the chat. A plugin ships a card schema, which is JSON data and not
//! code (`docs/capabilities/card-schema-v1.md`). This module reads a card, checks its limits, and turns a card plus a tool result
//! into plain resolved values. The UI draws those values with its own components, and so does the iOS app.
mod format;
mod resolve;
mod structs;
mod types;
mod validate;

pub use resolve::{failed_title, resolve, running_title};
pub use structs::{
    Action, Card, Condition, Done, Literal, Node, PathValue, Rendered, Resolved, ResolvedAction,
    Row, StateLine, TemplateValue, Value,
};
pub use types::{Direction, Format, Gap, Style, Tone};
pub use validate::{from_json, validate};

#[cfg(test)]
mod tests;
