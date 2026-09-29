//! lathe's own record of a session: with no external store, the conversation is kept with the project, in
//! `.lathe/agent/sessions/`, written through `Project::write` so a remote project keeps it on its host.
//! Two files per session: `<id>.jsonl` holds one message a line, and `<id>.meta` holds the title and the
//! time, small enough to read for a list. `history` turns the messages into the events a UI shows.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lathe_project::{Command, Project};
use serde::{Deserialize, Serialize};

use super::{
    message::{Block, Message, Role},
    tools,
};
use crate::{
    session::{BlockId, Event, SessionError, SessionId, SessionSummary, ToolCall, ToolId, ToolOutput, ToolStatus},
    subprocess,
};

pub const DIR: &str = ".lathe/agent/sessions";
const LIST_LIMIT: usize = 50;
const TITLE_CHARS: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub title: String,
    pub model: String,
    /// Seconds since the Unix epoch.
    pub updated: u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// An id is made of letters, digits and dashes, so it is never a path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn title_of(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() > TITLE_CHARS { format!("{}…", line.chars().take(TITLE_CHARS).collect::<String>()) } else { line.to_string() }
}

fn path(id: &str, ext: &str) -> String {
    format!("{DIR}/{id}.{ext}")
}

pub fn ensure_dir(project: &dyn Project) -> Result<(), String> {
    let target = project.root().join(DIR);
    let command = Command::new("mkdir").args(["-p", "--"]).args([target.to_string_lossy().into_owned()]);
    let mut process = project.spawn(&command).map_err(|e| e.to_string())?;
    drop(std::mem::replace(&mut process.stdin, Box::new(std::io::sink())));
    match process.control.wait() {
        Ok(Some(0)) => Ok(()),
        _ => Err(process.control.stderr().trim().to_string()),
    }
}

/// Writes the whole record. The messages first, then the meta, so a listed session always has its messages.
pub fn save(project: &dyn Project, meta: &Meta, messages: &[Message]) -> Result<(), String> {
    let mut lines = String::new();
    for message in messages {
        lines.push_str(&serde_json::to_string(message).map_err(|e| e.to_string())?);
        lines.push('\n');
    }
    project.write(&path(&meta.id, "jsonl"), lines.as_bytes()).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(meta).map_err(|e| e.to_string())?;
    project.write(&path(&meta.id, "meta"), &bytes).map_err(|e| e.to_string())
}

pub fn load(project: &dyn Project, id: &str) -> Result<(Meta, Vec<Message>), SessionError> {
    if !valid_id(id) {
        return Err(SessionError::Read(format!("{id} is not a session id")));
    }
    let read = |ext: &str| project.read(&path(id, ext)).map_err(|e| SessionError::Read(format!("session {id}: {e}")));
    let meta: Meta = serde_json::from_slice(&read("meta")?).map_err(|e| SessionError::Read(format!("session {id}: {e}")))?;
    let text = String::from_utf8_lossy(&read("jsonl")?).into_owned();
    let mut messages = Vec::new();
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        messages.push(serde_json::from_str::<Message>(line).map_err(|e| SessionError::Read(format!("session {id}, line {}: {e}", n + 1)))?);
    }
    Ok((meta, messages))
}

/// The project's sessions, newest first.
pub fn list(project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
    let script = format!("ls -t {DIR}/*.meta 2>/dev/null | head -n {LIST_LIMIT}");
    let command = Command::new("sh").args(["-c", script.as_str()]);
    let names = subprocess::output(project, &command)?;
    let mut out = Vec::new();
    for line in names.lines().filter(|l| !l.trim().is_empty()) {
        let Some(bytes) = line.trim().strip_prefix(&format!("{DIR}/")).and_then(|n| project.read(&format!("{DIR}/{n}")).ok()) else { continue };
        if let Ok(meta) = serde_json::from_slice::<Meta>(&bytes)
            && valid_id(&meta.id)
            && !meta.title.is_empty()
        {
            out.push(SessionSummary { id: SessionId::new(meta.id), title: meta.title, updated: Some(meta.updated) });
        }
    }
    Ok(out)
}

pub fn history(project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
    let (_, messages) = load(project, session.as_str())?;
    Ok(events_of(&messages))
}

/// The events that show a conversation already had: what the user said, what the model said, each tool
/// call with its result.
pub fn events_of(messages: &[Message]) -> Vec<Event> {
    let mut events = Vec::new();
    let mut next = 0u64;
    let mut calls: Vec<(String, String)> = Vec::new();
    for message in messages {
        for block in &message.blocks {
            match (message.role, block) {
                (Role::User, Block::Text { text }) => events.push(Event::UserMessage { text: text.clone() }),
                (Role::Assistant, Block::Text { text }) => {
                    events.push(Event::Text { block: BlockId(next), delta: text.clone() });
                    next += 1;
                }
                (Role::Assistant, Block::Thinking { text, .. }) if !text.is_empty() => {
                    events.push(Event::Thinking { block: BlockId(next), delta: text.clone() });
                    events.push(Event::ThinkingDone { block: BlockId(next), took: Duration::ZERO });
                    next += 1;
                }
                (Role::Assistant, Block::ToolUse { id, name, input }) => {
                    let (kind, file) = tools::describe(name, input);
                    events.push(Event::ToolStarted(ToolCall {
                        id: ToolId::new(id.clone()),
                        name: name.clone(),
                        kind,
                        input: input.clone(),
                        file,
                        parent: None,
                        status: ToolStatus::Running,
                    }));
                    calls.push((id.clone(), name.clone()));
                }
                (Role::User, Block::ToolResult { id, content, is_error }) => {
                    let truncated = content.len() > ToolOutput::MAX_TEXT;
                    let text = tools::cap(content, ToolOutput::MAX_TEXT);
                    events.push(Event::ToolFinished { id: ToolId::new(id.clone()), output: ToolOutput { text, is_error: *is_error, truncated, full_at: None } });
                }
                _ => {}
            }
        }
    }
    events
}
