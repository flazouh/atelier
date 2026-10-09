use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::types::{Direction, Format, Gap, Style, Tone};

/// What a plugin ships for one tool: three states, of which only `done` has a body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub version: u32,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<StateLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<StateLine>,
    pub done: Done,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateLine {
    pub title: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Done {
    pub title: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footer: Vec<Action>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Text(String),
    Number(serde_json::Number),
    Flag(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathValue {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Literal>,
}

/// Text with `{$.path}` holes. A hole may name a format: `{$.last_seen|relative_time}`. Nothing else is evaluated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateValue {
    pub template: String,
}

/// A literal, a path into the tool result, or a template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Path(PathValue),
    Template(TemplateValue),
    Literal(Literal),
}

/// A node shows only when its condition holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exists: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Open {
        label: String,
        #[serde(rename = "ref")]
        reference: Value,
    },
    OpenUrl {
        label: String,
        url: Value,
    },
    Call {
        label: String,
        tool: String,
        #[serde(default)]
        args: BTreeMap<String, Value>,
    },
}

/// The closed set of things a card is made of. A new kind needs a change to Atelier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    Stack {
        #[serde(default)]
        direction: Direction,
        #[serde(default)]
        gap: Gap,
        children: Vec<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Text {
        value: Value,
        #[serde(default)]
        style: Style,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_lines: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Badge {
        value: Value,
        #[serde(default)]
        tone: Tone,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Metric {
        label: String,
        value: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Icon {
        name: String,
        #[serde(default)]
        tone: Tone,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Avatar {
        name: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Code {
        value: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_lines: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    List {
        items: String,
        item: Box<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        empty: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_item: Option<Action>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Button {
        action: Action,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
    Divider {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Condition>,
    },
}

/// An action with its values read from the tool result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedAction {
    Open {
        label: String,
        #[serde(rename = "ref")]
        reference: String,
    },
    OpenUrl {
        label: String,
        url: String,
    },
    Call {
        label: String,
        tool: String,
        args: BTreeMap<String, String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub node: Resolved,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_click: Option<ResolvedAction>,
}

/// A node with every value read: plain text and plain choices, ready to draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resolved {
    Stack {
        direction: Direction,
        gap: Gap,
        children: Vec<Resolved>,
    },
    Text {
        value: String,
        style: Style,
        max_lines: Option<u8>,
    },
    Badge {
        value: String,
        tone: Tone,
    },
    Metric {
        label: String,
        value: String,
    },
    Icon {
        name: String,
        tone: Tone,
    },
    Avatar {
        name: String,
        url: Option<String>,
    },
    Code {
        value: String,
        language: Option<String>,
        max_lines: Option<u8>,
    },
    List {
        rows: Vec<Row>,
        hidden: usize,
        empty: Option<String>,
    },
    Button {
        action: ResolvedAction,
    },
    Divider,
}

/// A card in its `done` state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub title: String,
    pub body: Option<Resolved>,
    pub footer: Vec<ResolvedAction>,
}
