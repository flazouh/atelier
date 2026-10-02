use serde_json::Value;

use crate::session::ToolKind;
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
    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult;
}
