use std::time::Instant;

use atelier_project::{Command, Project};
use serde_json::Value;

use super::super::map::{ClaudeLineMapper, LineMapper};
use crate::{
    session::{Event, SessionError, SessionId, SessionSummary},
    subprocess,
};
use super::types::{LIST_LIMIT, MARK, TITLE_CHARS, TITLE_LINES};

/// The folder name `claude` gives a project.
pub(in super::super) fn slug(root: &str) -> String {
    root.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// The session files of the project `slug` in every account: `~/.claude`'s, then each `~/.claude-<name>`'s.
fn session_files(slug: &str, file: &str) -> String {
    format!(r#""$HOME/.claude/projects/{slug}"/{file} "$HOME"/.claude-*/projects/"{slug}"/{file}"#)
}

/// Prints the newest sessions of the project `slug`, each as a `MARK path mtime` line and the head of its file.
pub(in super::super) fn list_script(slug: &str) -> String {
    format!(
        r#"ls -t {files} 2>/dev/null | head -n {LIST_LIMIT} | while IFS= read -r f; do
  printf '{MARK}%s %s\n' "$f" "$(stat -c %Y "$f" 2>/dev/null || stat -f %m "$f")"
  head -n {TITLE_LINES} "$f" | cut -c1-4000
done"#,
        files = session_files(slug, "*.jsonl"),
    )
}

/// Prints the transcript of `session`, from whichever account holds it.
pub(in super::super) fn read_script(slug: &str, session: &SessionId) -> String {
    format!(
        r#"for f in {files}; do
  if [ -f "$f" ]; then exec cat "$f"; fi
done
exit 1"#,
        files = session_files(slug, &format!("{}.jsonl", session.as_str())),
    )
}

/// Prints the path of the file that holds `session`, from whichever account does.
pub(in super::super) fn holder_script(slug: &str, session: &SessionId) -> String {
    format!(
        r#"for f in {files}; do
  if [ -f "$f" ]; then printf '%s\n' "$f"; exit 0; fi
done"#,
        files = session_files(slug, &format!("{}.jsonl", session.as_str())),
    )
}

/// The named account that holds a session, from what [`holder_script`] printed: `None` for the usual account, and for
/// a session no account holds.
pub(in super::super) fn holder_account(printed: &str) -> Option<String> {
    account_of(printed.trim())
}

/// The named account that holds `session` on the project's host, `None` for the usual one.
pub(in super::super) fn holder(project: &dyn Project, session: &SessionId) -> Result<Option<String>, SessionError> {
    if !session.as_str().chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(SessionError::Read("the session id has characters a file name cannot".into()));
    }
    let script = holder_script(&slug(&project.root().to_string_lossy()), session);
    let command = Command::new("sh").args(["-c", script.as_str()]);
    Ok(holder_account(&subprocess::output(project, &command)?))
}

pub(in super::super) fn list(project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
    let root = project.root().to_string_lossy().into_owned();
    let script = list_script(&slug(&root));
    let command = Command::new("sh").args(["-c", script.as_str()]);
    Ok(parse_listing_in(&subprocess::output(project, &command)?, &root))
}

/// The account a session file was saved under: `None` for `~/.claude`, `work` for `~/.claude-work`.
fn account_of(path: &str) -> Option<String> {
    let mut folders = path.rsplit('/').skip(1);
    let (_project, projects, config) = (folders.next()?, folders.next()?, folders.next()?);
    if projects != "projects" {
        return None;
    }
    config.strip_prefix(".claude-").map(str::to_string)
}

/// Reads the listing script's output: a `@@ path mtime` line, then the head of that file. Every file, whatever folder it ran in.
#[cfg(test)]
pub(in super::super) fn parse_listing(text: &str) -> Vec<SessionSummary> {
    parse(text).into_iter().map(|(session, _)| session).collect()
}

/// The sessions of the project at `root`. `claude` names a project's folder by turning every character that is not a
/// letter or a digit into `-`, so two folders can share one; a transcript records the folder it ran in, and a session
/// that ran in another is left out. A transcript that records none stays.
pub(in super::super) fn parse_listing_in(text: &str, root: &str) -> Vec<SessionSummary> {
    parse(text)
        .into_iter()
        .filter(|(_, cwd)| cwd.as_deref().is_none_or(|cwd| same_folder(cwd, root)))
        .map(|(session, _)| session)
        .collect()
}

/// Whether two paths name one folder, as macOS does for `/tmp/x` and `/private/tmp/x`.
fn same_folder(a: &str, b: &str) -> bool {
    let plain = |path: &str| {
        let path = path.trim_end_matches('/');
        path.strip_prefix("/private").filter(|rest| rest.starts_with('/')).unwrap_or(path).to_string()
    };
    plain(a) == plain(b)
}

fn parse(text: &str) -> Vec<(SessionSummary, Option<String>)> {
    let mut sessions: Vec<(SessionSummary, Option<String>)> = Vec::new();
    let mut titled = true;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix(MARK) {
            let (path, updated) = header.rsplit_once(' ').unwrap_or((header, ""));
            let id = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".jsonl");
            sessions.push((SessionSummary { id: SessionId::new(id), title: String::new(), updated: updated.parse().ok(), account: account_of(path) }, None));
            titled = false;
            continue;
        }
        let Some((session, cwd)) = sessions.last_mut() else { continue };
        if cwd.is_none() {
            *cwd = folder_of(line);
        }
        if !titled
            && let Some(text) = user_text(line)
        {
            session.title = cut(&text);
            titled = true;
        }
    }
    sessions.retain(|(session, _)| !session.title.is_empty());
    sessions
}

/// The folder a transcript line says the session ran in.
fn folder_of(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    value.get("cwd")?.as_str().map(str::to_string)
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

pub(in super::super) fn read_history(project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
    if !session.as_str().chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(SessionError::Read("the session id has characters a file name cannot".into()));
    }
    let script = read_script(&slug(&project.root().to_string_lossy()), session);
    let command = Command::new("sh").args(["-c", script.as_str()]);
    Ok(history(&subprocess::output(project, &command)?))
}

/// A transcript as events, as if the session had just run. Nothing streams, so text comes whole and
/// thinking has no time.
pub fn history(transcript: &str) -> Vec<Event> {
    let mut mapper = ClaudeLineMapper::new();
    let now = Instant::now();
    let mut events: Vec<Event> = transcript.lines().flat_map(|line| mapper.line(line, now)).collect();
    events.extend(mapper.end_of_history());
    events
}
