use serde_json::{Value, json};

use super::{Access, Tool, ToolContext, ToolResult, make_parent, object, project_path};
use crate::session::ToolKind;

pub struct Write;

impl Tool for Write {
    fn name(&self) -> &'static str {
        "write"
    }

    fn description(&self) -> &'static str {
        "Writes a file of the project, whole. It makes the folder if needed and replaces a file that exists. To change part of a file, use `edit`."
    }

    fn schema(&self) -> Value {
        object(
            json!({
                "path": {"type": "string", "description": "The file, relative to the project root."},
                "content": {"type": "string", "description": "The whole new content of the file."}
            }),
            &["path", "content"],
        )
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Write
    }

    fn access(&self) -> Access {
        Access::Edit
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let path = match project_path(ctx, input, "path") {
            Ok(path) => path,
            Err(e) => return e,
        };
        let Some(content) = input["content"].as_str() else { return ToolResult::err("`content` is required and must be a string") };
        if let Err(e) = make_parent(ctx, path) {
            return ToolResult::err(e);
        }
        match ctx.project.write(path, content.as_bytes()) {
            Ok(()) => ToolResult::ok(format!("Wrote {} bytes to {path}", content.len())),
            Err(e) => ToolResult::err(format!("cannot write {path}: {e}")),
        }
    }
}
