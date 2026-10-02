use atelier_project::Project;

use super::super::message::Cancel;

pub struct ToolContext<'a> {
    pub project: &'a dyn Project,
    pub cancel: &'a Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: false }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: true }
    }
}
