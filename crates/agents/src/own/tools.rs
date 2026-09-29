//! The agent's tools. Each is a function of the project and a JSON input, and touches the project only
//! through the `Project` interface, so the same tool works on a folder here and on an SSH host.
//! A tool never panics on bad input: it returns an error the model can read and fix.
use std::{fmt::Write as _, path::Path};

use lathe_project::{Project, host_path};
use serde_json::{Value, json};

use super::message::{Cancel, ToolDef};
use crate::session::ToolKind;

mod edit;
mod list;
mod read;
mod search;
mod shell;
mod write;

/// What a call does to the project. Permission modes decide by this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Looks only.
    Read,
    /// Changes files.
    Edit,
    /// Runs a process, which may do anything.
    Execute,
}

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

/// The most text a tool returns to the model. A longer result keeps its head and says so.
pub const MAX_RESULT: usize = 30_000;

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
            Box::new(read::Read),
            Box::new(list::List),
            Box::new(search::Search),
            Box::new(edit::Edit),
            Box::new(write::Write),
            Box::new(shell::Shell),
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
    let command = lathe_project::Command::new("mkdir").args(["-p", "--"]).args([target.to_string_lossy().into_owned()]);
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

pub(crate) fn name_of(path: &str) -> &str {
    Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)
}

/// The kind and the file of a call of `name`, for a history that shows calls without running a tool.
pub fn describe(name: &str, input: &Value) -> (ToolKind, Option<String>) {
    match all().iter().find(|t| t.name() == name) {
        Some(tool) => (tool.kind(), tool.file(input)),
        None => (ToolKind::Other, None),
    }
}
