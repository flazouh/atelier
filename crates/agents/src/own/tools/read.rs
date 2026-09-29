use serde_json::{Value, json};

use super::{MAX_RESULT, Tool, ToolContext, ToolResult, Access, object, project_path};
use crate::session::ToolKind;

const DEFAULT_LINES: usize = 2000;

pub struct Read;

impl Tool for Read {
    fn name(&self) -> &'static str {
        "read"
    }

    fn description(&self) -> &'static str {
        "Reads a text file of the project. Returns its lines, each with its number. Use `offset` (first line, from 1) and `limit` (line count) for a long file."
    }

    fn schema(&self) -> Value {
        object(
            json!({
                "path": {"type": "string", "description": "The file, relative to the project root."},
                "offset": {"type": "integer", "description": "The first line to return, from 1."},
                "limit": {"type": "integer", "description": "How many lines to return."}
            }),
            &["path"],
        )
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn access(&self) -> Access {
        Access::Read
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let path = match project_path(ctx, input, "path") {
            Ok(path) => path,
            Err(e) => return e,
        };
        let bytes = match ctx.project.read(path) {
            Ok(bytes) => bytes,
            Err(e) => return ToolResult::err(format!("cannot read {path}: {e}")),
        };
        if bytes.iter().take(8000).any(|b| *b == 0) {
            return ToolResult::err(format!("{path} is a binary file"));
        }
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        let first = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().map_or(DEFAULT_LINES, |l| (l as usize).clamp(1, DEFAULT_LINES));
        if lines.is_empty() {
            return ToolResult::ok(format!("{path} is empty"));
        }
        if first > lines.len() {
            return ToolResult::err(format!("{path} has {} lines; offset {first} is past the end", lines.len()));
        }
        let mut out = String::new();
        let mut shown = 0;
        for (n, line) in lines.iter().enumerate().skip(first - 1).take(limit) {
            let numbered = format!("{}\t{}\n", n + 1, line);
            if out.len() + numbered.len() > MAX_RESULT {
                break;
            }
            out.push_str(&numbered);
            shown += 1;
        }
        let last = first - 1 + shown;
        if last < lines.len() {
            out.push_str(&format!("[showing lines {first} to {last} of {}; use offset {} to go on]\n", lines.len(), last + 1));
        }
        ToolResult::ok(out)
    }
}
