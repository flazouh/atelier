use serde_json::{Value, json};

use super::{Access, Tool, ToolContext, ToolResult, object};
use crate::session::ToolKind;

const MAX_ENTRIES: usize = 500;

pub struct List;

impl Tool for List {
    fn name(&self) -> &'static str {
        "list"
    }

    fn description(&self) -> &'static str {
        "Lists the files and folders of the project, or of one folder in it, all levels down. Files that .gitignore hides are left out."
    }

    fn schema(&self) -> Value {
        object(json!({"path": {"type": "string", "description": "A folder, relative to the project root. The whole project when left out."}}), &[])
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Search
    }

    fn access(&self) -> Access {
        Access::Read
    }

    fn file(&self, _: &Value) -> Option<String> {
        None
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let prefix = match input.get("path").and_then(Value::as_str).filter(|p| !p.is_empty() && *p != ".") {
            Some(_) => match super::project_path(ctx, input, "path") {
                Ok(path) => format!("{}/", path.trim_end_matches('/')),
                Err(e) => return e,
            },
            None => String::new(),
        };
        let entries = match ctx.project.list() {
            Ok(entries) => entries,
            Err(e) => return ToolResult::err(format!("cannot list the project: {e}")),
        };
        let mut out = String::new();
        let mut shown = 0;
        let mut total = 0;
        for entry in entries.iter().filter(|e| e.path.starts_with(&prefix)) {
            total += 1;
            if shown < MAX_ENTRIES {
                out.push_str(&entry.path);
                if entry.dir {
                    out.push('/');
                }
                out.push('\n');
                shown += 1;
            }
        }
        if total == 0 {
            return ToolResult::err(if prefix.is_empty() { "the project has no files".into() } else { format!("nothing under {prefix}") });
        }
        if total > shown {
            out.push_str(&format!("[{shown} of {total} entries shown; list a folder to see the rest]\n"));
        }
        ToolResult::ok(out)
    }
}
