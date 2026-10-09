use serde_json::{Value, json};

use super::types::Permission;

/// One tool as `tools/list` shows it.
#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: String,
    pub title: String,
    pub description: String,
    /// A JSON Schema of the arguments, an object.
    pub input_schema: Value,
    pub permission: Permission,
    /// The tool reaches a service outside the app (Slack, Gmail). Tasks tools stay in the app's own data.
    pub open_world: bool,
}

impl ToolDef {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.permission.read_only(),
                "destructiveHint": self.permission.destructive(),
                "openWorldHint": self.open_world,
            },
        })
    }
}

/// What a tool call gives back: a short text for the model, and the neutral data for a client that reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    pub text: String,
    /// An object, when there is data.
    pub structured: Option<Value>,
    /// The call failed in a way the model can read and act on. The transport worked, so this is not a protocol error.
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(text: impl Into<String>, structured: Value) -> Self {
        Self {
            text: text.into(),
            structured: Some(structured),
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            structured: None,
            is_error: true,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut result = json!({
            "content": [{ "type": "text", "text": self.text }],
            "isError": self.is_error,
        });
        if let Some(structured) = &self.structured {
            result["structuredContent"] = structured.clone();
        }
        result
    }
}
