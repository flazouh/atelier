use std::fmt::Write as _;

use atelier_project::host_path;
use serde_json::{Value, json};

use super::super::message::ToolDef;
use crate::session::ToolKind;
use super::structs::{ToolContext, ToolResult};
use super::traits::Tool;

/// `text` cut at `max` bytes on a character edge, with a note when it was cut.
pub fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text[..end].to_string();
    let _ = write!(out, "\n[output cut at {max} bytes; {} more bytes not shown]", text.len() - end);
    out
}

/// Every tool the agent has, in the order the model sees them.
pub fn all() -> &'static [Box<dyn Tool>] {
    static TOOLS: std::sync::OnceLock<Vec<Box<dyn Tool>>> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| {
        vec![
            Box::new(super::read::Read),
            Box::new(super::list::List),
            Box::new(super::search::Search),
            Box::new(super::edit::Edit),
            Box::new(super::write::Write),
            Box::new(super::shell::Shell),
        ]
    })
}

pub fn definitions(tools: &[Box<dyn Tool>]) -> Vec<ToolDef> {
    tools.iter().map(|t| ToolDef { name: t.name().into(), description: t.description().into(), schema: t.schema() }).collect()
}

/// A root-relative path from the model, checked to be inside the project.
pub(crate) fn project_path<'a>(ctx: &ToolContext<'_>, input: &'a Value, key: &str) -> Result<&'a str, ToolResult> {
    let path = input[key].as_str().ok_or_else(|| ToolResult::err(format!("`{key}` is required and must be a string")))?;
    let path = path.trim_start_matches("./");
    // A path the model copied from a message may be absolute, inside the project.
    let root = ctx.project.root().to_string_lossy().into_owned();
    let path = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))).unwrap_or(path);
    host_path(ctx.project.root(), path).map_err(|_| ToolResult::err(format!("{path} is not inside the project")))?;
    Ok(path)
}

/// Makes the folder a file goes in, on the project's host.
pub(crate) fn make_parent(ctx: &ToolContext<'_>, path: &str) -> Result<(), String> {
    let Some((parent, _)) = path.rsplit_once('/') else { return Ok(()) };
    let target = host_path(ctx.project.root(), parent).map_err(|e| e.to_string())?;
    let command = atelier_project::Command::new("mkdir").args(["-p", "--"]).args([target.to_string_lossy().into_owned()]);
    let mut process = ctx.project.spawn(&command).map_err(|e| format!("cannot make the folder: {e}"))?;
    drop(std::mem::replace(&mut process.stdin, Box::new(std::io::sink())));
    match process.control.wait() {
        Ok(Some(0)) => Ok(()),
        _ => Err(format!("cannot make the folder {parent}: {}", process.control.stderr().trim())),
    }
}

/// The tool input as a compact line for a log or a permission prompt.
pub fn brief(input: &Value) -> String {
    let text = input.to_string();
    if text.chars().count() > 200 { format!("{}…", text.chars().take(200).collect::<String>()) } else { text }
}

/// A schema object with these properties, all in `required` unless listed in `optional`.
pub(crate) fn object(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required})
}

/// The kind and the file of a call of `name`, for a history that shows calls without running a tool.
pub fn describe(name: &str, input: &Value) -> (ToolKind, Option<String>) {
    match all().iter().find(|t| t.name() == name) {
        Some(tool) => (tool.kind(), tool.file(input)),
        None => (ToolKind::Other, None),
    }
}
