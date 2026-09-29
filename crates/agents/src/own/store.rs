//! lathe's own record of a session: with no external store, the conversation is kept in the project's data
//! folder (`Project::data_write`), outside the repository and on the project's host, so it never shows in
//! `git status`, in `agent/sessions/`. Sessions written by an older lathe into the project's own
//! `.lathe/agent/sessions/` are moved there the first time the list is read ([`migrate`]).
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

/// Where the record lives, in the data folder.
pub const DIR: &str = "agent/sessions";
/// Where an older lathe wrote it, in the project itself.
const LEGACY_DIR: &str = ".lathe/agent/sessions";
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

/// Moves the record an older lathe left in the project (`.lathe/agent/sessions/`) into the data folder, and
/// removes what it moved. Once a session is in both places the one in the data folder wins. Gives the number
/// of sessions moved. A project with nothing to move costs one `ls`.
pub fn migrate(project: &dyn Project) -> Result<usize, String> {
    let command = Command::new("sh").args(["-c", "ls -1 -- \"$1\" 2>/dev/null; true", "sh", LEGACY_DIR]);
    let names = subprocess::output(project, &command).map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = names
        .lines()
        .filter_map(|n| n.trim().strip_suffix(".meta").or_else(|| n.trim().strip_suffix(".jsonl")))
        .filter(|id| valid_id(id))
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    let mut moved = 0;
    for id in ids {
        // A session with no messages is not a session: leave it be. The messages go first, then the meta, as
        // `save` does, so a listed session always has its messages.
        let (jsonl, meta) = (format!("{LEGACY_DIR}/{id}.jsonl"), format!("{LEGACY_DIR}/{id}.meta"));
        let (Ok(lines), Ok(info)) = (project.read(&jsonl), project.read(&meta)) else { continue };
        for (ext, bytes, old) in [("jsonl", lines, jsonl), ("meta", info, meta)] {
            if project.data_read(&path(&id, ext)).is_err() {
                project.data_write(&path(&id, ext), &bytes).map_err(|e| e.to_string())?;
            }
            let _ = project.remove(&old);
        }
        let whole = true;
        moved += usize::from(whole);
    }
    // The folders are left behind empty: take them away, but only while they are empty.
    let tidy = Command::new("sh").args(["-c", "rmdir -- \"$1\" .lathe/agent .lathe 2>/dev/null; true", "sh", LEGACY_DIR]);
    let _ = subprocess::output(project, &tidy);
    Ok(moved)
}

/// Writes the whole record. The messages first, then the meta, so a listed session always has its messages.
pub fn save(project: &dyn Project, meta: &Meta, messages: &[Message]) -> Result<(), String> {
    let mut lines = String::new();
    for message in messages {
        lines.push_str(&serde_json::to_string(message).map_err(|e| e.to_string())?);
        lines.push('\n');
    }
    project.data_write(&path(&meta.id, "jsonl"), lines.as_bytes()).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(meta).map_err(|e| e.to_string())?;
    project.data_write(&path(&meta.id, "meta"), &bytes).map_err(|e| e.to_string())
}

pub fn load(project: &dyn Project, id: &str) -> Result<(Meta, Vec<Message>), SessionError> {
    if !valid_id(id) {
        return Err(SessionError::Read(format!("{id} is not a session id")));
    }
    let read = |ext: &str| {
        project.data_read(&path(id, ext)).or_else(|first| {
            // A session an older lathe wrote is moved when the list is read; one asked for by id first is moved now.
            if first.kind() == std::io::ErrorKind::NotFound && migrate(project).is_ok() { project.data_read(&path(id, ext)) } else { Err(first) }
        })
        .map_err(|e| SessionError::Read(format!("session {id}: {e}")))
    };
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
    // A record left by an older lathe comes into the data folder first. A failure to move it is not one to list.
    let _ = migrate(project);
    let entries = match project.data_list(DIR) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(SessionError::Read(error.to_string())),
    };
    let mut out = Vec::new();
    for entry in entries.iter().filter(|e| e.path.ends_with(".meta")).take(LIST_LIMIT) {
        let Ok(bytes) = project.data_read(&entry.path) else { continue };
        if let Ok(meta) = serde_json::from_slice::<Meta>(&bytes)
            && valid_id(&meta.id)
            && !meta.title.is_empty()
        {
            out.push(SessionSummary { id: SessionId::new(meta.id), title: meta.title, updated: Some(meta.updated) });
        }
    }
    // A moved session has the time of its move; the time in its meta is the one that says when it was last used.
    out.sort_by_key(|s| std::cmp::Reverse(s.updated));
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
