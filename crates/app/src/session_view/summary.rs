//! What a tool call is about, in words a reader takes in at a glance: a title ("Ran", "Read", "Searched"), the
//! argument that says which (the command, the file, the pattern) and an icon for its kind. The agent gives a call a
//! name and a bag of arguments; "Bash" alone says nothing, but "Ran `cargo test -p atelier-ui`" does. Pure.
use atelier_agents::session::{Call, ToolKind, ToolStatus};
use serde_json::Value;

/// How a call reads in its row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// "Ran", "Read", "Searched": past tense once it has ended, the "-ing" form while it runs.
    pub title: String,
    /// The argument that says which one, short: the command's first line, the pattern, the address.
    pub detail: Option<String>,
    /// The file the call is about, when it names one, which the row draws with its own icon.
    pub file: Option<String>,
    pub kind: ToolKind,
}

/// The most characters of an argument a row keeps; the row cuts what is left with an ellipsis of its own.
const DETAIL_MAX: usize = 160;

fn text<'a>(input: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| input.get(*k).and_then(Value::as_str)).map(str::trim).filter(|s| !s.is_empty())
}

/// The first line of `s`, cut to [`DETAIL_MAX`] characters.
fn first_line(s: &str) -> String {
    let line = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() <= DETAIL_MAX {
        return line.to_string();
    }
    let mut cut: String = line.chars().take(DETAIL_MAX - 1).collect();
    cut.push('…');
    cut
}

/// The verb for a call that is `running` or has ended, from the agent's name for the tool and the kind.
fn verb(name: &str, kind: ToolKind, running: bool) -> &'static str {
    let (now, done) = match (name, kind) {
        ("Bash", _) | (_, ToolKind::Shell) => ("Running", "Ran"),
        ("Read", _) | (_, ToolKind::Read) => ("Reading", "Read"),
        ("Edit" | "MultiEdit" | "NotebookEdit", _) | (_, ToolKind::Edit) => ("Editing", "Edited"),
        ("Write", _) | (_, ToolKind::Write) => ("Writing", "Wrote"),
        ("Grep", _) => ("Searching", "Searched"),
        ("Glob", _) => ("Finding files", "Found files"),
        (_, ToolKind::Search) => ("Searching", "Searched"),
        ("WebFetch", _) | (_, ToolKind::Fetch) => ("Fetching", "Fetched"),
        _ => ("Running", "Ran"),
    };
    if running { now } else { done }
}

/// The summary of `call`: `root` is the project's folder, taken off a path to show it as the project knows it.
pub fn summary(call: &Call, relative: impl Fn(&str) -> String) -> Summary {
    let c = &call.call;
    let running = matches!(c.status, ToolStatus::Pending | ToolStatus::Running);
    let name = c.name.as_str();
    let input = &c.input;
    let kind = c.kind;
    let mut title = verb(name, kind, running).to_string();
    let mut file = c.file.as_deref().map(&relative);
    let detail = match name {
        // The description the agent wrote is the title, what the call is for; the command, which says what it is, is the argument.
        "Bash" => match (text(input, &["description"]), text(input, &["command"])) {
            (Some(why), Some(command)) => {
                title = first_line(why);
                Some(first_line(command))
            }
            (None, Some(command)) => Some(first_line(command)),
            (Some(why), None) => Some(first_line(why)),
            (None, None) => None,
        },
        "Grep" => text(input, &["pattern"]).map(|p| match text(input, &["path"]) {
            Some(path) => format!("{} in {}", first_line(p), relative(path)),
            None => first_line(p),
        }),
        "Glob" => text(input, &["pattern"]).map(first_line),
        "WebFetch" => text(input, &["url"]).map(first_line),
        "WebSearch" => {
            title = if running { "Searching the web".into() } else { "Searched the web".into() };
            text(input, &["query"]).map(first_line)
        }
        "Task" | "Agent" => text(input, &["description", "prompt"]).map(first_line),
        _ => {
            if file.is_none() {
                file = text(input, &["file_path", "path", "notebook_path"]).map(&relative);
            }
            if file.is_none() { text(input, &["command", "pattern", "query", "url", "description"]).map(first_line) } else { None }
        }
    };
    if file.is_none() && matches!(name, "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
        file = text(input, &["file_path", "notebook_path"]).map(&relative);
    }
    // A call that names a tool this build does not know keeps the tool's own name as its title.
    if !matches!(name, "Bash" | "Read" | "Edit" | "MultiEdit" | "NotebookEdit" | "Write" | "Grep" | "Glob" | "WebFetch" | "WebSearch")
        && matches!(kind, ToolKind::Other)
    {
        title = name.to_string();
    }
    Summary { title, detail, file, kind }
}

#[cfg(test)]
mod tests;
