use serde_json::Value;

use crate::session::{FileEdit, ToolKind};
use super::structs::{ToolContext, ToolResult};
use super::types::Access;

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// A JSON schema for the input.
    fn schema(&self) -> Value;
    fn kind(&self) -> ToolKind;
    fn access(&self) -> Access;
    /// The file a call names, for the review that wants it before the tool runs.
    fn file(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_string)
    }
    /// The text the call changes, for a tool that edits or writes a file; the input can be one still streaming in, with
    /// a text not there yet left empty.
    fn edit(&self, _input: &Value) -> Option<FileEdit> {
        None
    }
    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult;
}
