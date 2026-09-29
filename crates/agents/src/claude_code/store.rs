//! Claude Code's own record of past sessions: one JSON-lines file per session in
//! `~/.claude/projects/<the project's folder, with every other character as "-">/`. The files live on the
//! project's host, so they are read through a process the project spawns, never straight from disk.
use std::time::Instant;

use lathe_project::{Command, Project};
use serde_json::Value;

use super::map::Mapper;
use crate::{
    session::{Event, SessionError, SessionId, SessionSummary},
    subprocess,
};

/// How many sessions the list holds.
const LIST_LIMIT: usize = 50;
/// How many lines of each file the list reads to find its title.
const TITLE_LINES: usize = 40;
const TITLE_CHARS: usize = 100;
const MARK: &str = "@@ ";

/// The folder name `claude` gives a project.
pub(super) fn slug(root: &str) -> String {
    root.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn folder(project: &dyn Project) -> String {
    format!("$HOME/.claude/projects/{}", slug(&project.root().to_string_lossy()))
}

pub(super) fn list(project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
    let script = format!(
        r#"ls -t "{dir}"/*.jsonl 2>/dev/null | head -n {LIST_LIMIT} | while IFS= read -r f; do
  printf '{MARK}%s %s\n' "$f" "$(stat -c %Y "$f" 2>/dev/null || stat -f %m "$f")"
  head -n {TITLE_LINES} "$f" | cut -c1-4000
done"#,
        dir = folder(project),
    );
    let command = Command::new("sh").args(["-c", script.as_str()]);
    Ok(parse_listing(&subprocess::output(project, &command)?))
}

/// Reads the listing script's output: a `@@ path mtime` line, then the head of that file.
pub(super) fn parse_listing(text: &str) -> Vec<SessionSummary> {
    let mut sessions: Vec<SessionSummary> = Vec::new();
    let mut titled = true;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix(MARK) {
            let (path, updated) = header.rsplit_once(' ').unwrap_or((header, ""));
            let id = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".jsonl");
            sessions.push(SessionSummary { id: SessionId::new(id), title: String::new(), updated: updated.parse().ok() });
            titled = false;
        } else if !titled
            && let Some((session, text)) = sessions.last_mut().zip(user_text(line))
        {
            session.title = cut(&text);
            titled = true;
        }
    }
    sessions.retain(|session| !session.title.is_empty());
    sessions
}

/// What the user typed, from one transcript line, when it is a user message.
fn user_text(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "user" || value.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let content = value.get("message")?.get("content")?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks.iter().find_map(|b| b.get("text")?.as_str()).map(str::to_string)?,
        _ => return None,
    };
    let text = text.trim();
    // Commands and their output are recorded as user lines wrapped in tags.
    (!text.is_empty() && !text.starts_with('<')).then(|| text.to_string())
}

fn cut(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= TITLE_CHARS {
        return one_line;
    }
    let mut cut: String = one_line.chars().take(TITLE_CHARS - 1).collect();
    cut.push('…');
    cut
}

pub(super) fn read_history(project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
    if !session.as_str().chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(SessionError::Read("the session id has characters a file name cannot".into()));
    }
    let script = format!("cat \"{}/{}.jsonl\"", folder(project), session.as_str());
    let command = Command::new("sh").args(["-c", script.as_str()]);
    Ok(history(&subprocess::output(project, &command)?))
}

/// A transcript as events, as if the session had just run. Nothing streams, so text comes whole and
/// thinking has no time.
pub fn history(transcript: &str) -> Vec<Event> {
    let mut mapper = Mapper::new();
    let now = Instant::now();
    let mut events: Vec<Event> = transcript.lines().flat_map(|line| mapper.line(line, now)).collect();
    events.extend(mapper.end_of_history());
    events
}
