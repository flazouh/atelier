use serde_json::{Value, json};

use super::{Access, Tool, ToolContext, ToolResult, object, project_path};
use crate::session::{FileEdit, ToolKind};

pub struct Edit;

impl Tool for Edit {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Changes a file by exact replacement: `old_string` must appear in the file exactly once (spaces and line ends included), and is replaced by `new_string`. If it appears more than once, add lines around it until it is unique, or set `replace_all`. Read the file first."
    }

    fn schema(&self) -> Value {
        object(
            json!({
                "path": {"type": "string", "description": "The file, relative to the project root."},
                "old_string": {"type": "string", "description": "The text to replace, exactly as it is in the file."},
                "new_string": {"type": "string", "description": "The text to put in its place."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence."}
            }),
            &["path", "old_string", "new_string"],
        )
    }

    fn edit(&self, input: &Value) -> Option<FileEdit> {
        let text = |key: &str| input[key].as_str().unwrap_or("").to_string();
        Some(FileEdit { path: self.file(input)?, old: text("old_string"), new: text("new_string") })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Edit
    }

    fn access(&self) -> Access {
        Access::Edit
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let path = match project_path(ctx, input, "path") {
            Ok(path) => path,
            Err(e) => return e,
        };
        let (Some(old), Some(new)) = (input["old_string"].as_str(), input["new_string"].as_str()) else {
            return ToolResult::err("`old_string` and `new_string` are required and must be strings");
        };
        if old.is_empty() {
            return ToolResult::err("`old_string` is empty; to make a new file, use `write`");
        }
        if old == new {
            return ToolResult::err("`old_string` and `new_string` are the same, so nothing would change");
        }
        let bytes = match ctx.project.read(path) {
            Ok(bytes) => bytes,
            Err(e) => return ToolResult::err(format!("cannot read {path}: {e}")),
        };
        let Ok(text) = String::from_utf8(bytes) else { return ToolResult::err(format!("{path} is not a UTF-8 text file")) };
        let count = text.matches(old).count();
        let all = input["replace_all"].as_bool().unwrap_or(false);
        if count == 0 {
            return ToolResult::err(format!("`old_string` was not found in {path}. Read the file again and copy the text exactly."));
        }
        if count > 1 && !all {
            return ToolResult::err(format!("`old_string` appears {count} times in {path}. Add lines around it to make it unique, or set `replace_all`."));
        }
        let changed = if all { text.replace(old, new) } else { text.replacen(old, new, 1) };
        match ctx.project.write(path, changed.as_bytes()) {
            Ok(()) => ToolResult::ok(format!("Edited {path}: {count} replacement{}", if count == 1 { "" } else { "s" })),
            Err(e) => ToolResult::err(format!("cannot write {path}: {e}")),
        }
    }
}
