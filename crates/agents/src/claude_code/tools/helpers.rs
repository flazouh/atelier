use serde_json::Value;

use crate::session::ToolKind;
use super::types::{FILE_KEYS, TodoTool};

pub(in super::super) fn kind(name: &str) -> ToolKind {
    match name {
        "Read" => ToolKind::Read,
        "Edit" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "Write" => ToolKind::Write,
        "Grep" | "Glob" => ToolKind::Search,
        "Bash" | "BashOutput" | "KillShell" => ToolKind::Shell,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

/// Whether a tool's input is text the reader can watch being written: an edit's old and new text, a file's content.
pub(in super::super) fn streams_input(name: &str) -> bool {
    matches!(name, "Edit" | "Write")
}

/// The file a call names, from the argument the tool uses for it.
pub(in super::super) fn file(input: &Value) -> Option<String> {
    FILE_KEYS.iter().find_map(|key| input.get(key)?.as_str()).map(str::to_string)
}

/// The file a call names, read from the start of its input while the JSON still streams in. Only a key
/// of the outermost object counts, so the same words inside a string (a file's new text) do not, and
/// only a value whose closing quote has arrived.
pub(in super::super) fn file_in_partial_input(json: &str) -> Option<String> {
    let bytes = json.as_bytes();
    let (mut depth, mut i) = (0usize, 0usize);
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            b'"' => {
                let end = string_end(bytes, i)?;
                if depth == 1 {
                    let key: String = serde_json::from_str(&json[i..=end]).ok()?;
                    let after = json[end + 1..].trim_start();
                    if FILE_KEYS.contains(&key.as_str()) && after.starts_with(':') {
                        let value = after[1..].trim_start();
                        if value.starts_with('"') {
                            let stop = string_end(value.as_bytes(), 0)?;
                            return serde_json::from_str(&value[..=stop]).ok();
                        }
                        return None;
                    }
                }
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The index of the quote that closes the string opening at `start`, or `None` while it is still open.
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'"' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The tools that start a subagent.
pub(in super::super) fn starts_subagent(name: &str) -> bool {
    matches!(name, "Agent" | "Task")
}

pub(in super::super) fn todo_tool(name: &str) -> Option<TodoTool> {
    match name {
        "TodoWrite" => Some(TodoTool::Write),
        "TaskCreate" => Some(TodoTool::Create),
        "TaskUpdate" => Some(TodoTool::Update),
        _ => None,
    }
}

/// The text fields of a call's input, read from the start of its JSON while it still streams in: a value that has closed
/// whole, the last one cut where the stream stopped. Only the outermost object's string values count; the others (a flag, a
/// list) wait for the whole input. `None` until a field has begun.
pub(in super::super) fn partial_input(json: &str) -> Option<Value> {
    let bytes = json.as_bytes();
    let skip = |mut i: usize| {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        i
    };
    let mut i = skip(0);
    if bytes.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut fields = serde_json::Map::new();
    loop {
        i = skip(i);
        if bytes.get(i) != Some(&b'"') {
            break;
        }
        let Some(end) = string_end(bytes, i) else { break };
        let Ok(key) = serde_json::from_str::<String>(&json[i..=end]) else { break };
        i = end + 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            break;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => break,
            Some(b'"') => match string_end(bytes, i) {
                Some(end) => {
                    let Ok(text) = serde_json::from_str::<String>(&json[i..=end]) else { break };
                    fields.insert(key, Value::String(text));
                    i = end + 1;
                }
                None => {
                    fields.insert(key, Value::String(cut_string(&json[i + 1..])));
                    break;
                }
            },
            Some(_) => match value_end(bytes, i) {
                Some(end) => i = end,
                None => break,
            },
        }
    }
    (!fields.is_empty()).then_some(Value::Object(fields))
}

/// The end of the value that is not a string and starts at `start`: the `,` or `}` that follows it, or `None` while it is open.
fn value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let (mut depth, mut i) = (0usize, start);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = string_end(bytes, i)?,
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth == 0 => return Some(i),
            b'}' | b']' => depth -= 1,
            b',' if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The text of a JSON string whose closing quote has not come: `raw` with a cut escape left off, unescaped.
fn cut_string(raw: &str) -> String {
    let mut raw = raw;
    // A backslash that ends the text starts an escape that has not arrived.
    let slashes = raw.bytes().rev().take_while(|&b| b == b'\\').count();
    if slashes % 2 == 1 {
        raw = &raw[..raw.len() - 1];
    }
    // So does a `\u` with fewer than four digits after it.
    if let Some(at) = raw.rfind("\\u")
        && raw[..at].bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 0
        && raw[at + 2..].len() < 4
    {
        raw = &raw[..at];
    }
    // A high surrogate waits for its pair.
    loop {
        if let Ok(text) = serde_json::from_str::<String>(&format!("\"{raw}\"")) {
            return text;
        }
        match raw.len().checked_sub(6) {
            Some(at) if raw.is_char_boundary(at) && raw[at..].starts_with("\\u") => raw = &raw[..at],
            _ => return String::new(),
        }
    }
}
