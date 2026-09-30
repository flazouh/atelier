//! What the agent is asked to draft (a commit message, a branch name), and what is made of its answer.
//! The agent sees the kept change as a diff; its answer is cleaned of fences and quotes, and a branch
//! name is made one git takes. Pure.

use lathe_review::Merged;

use crate::ship::kept::Kept;

/// The most of the diff an agent is sent: enough for a message, little enough for a quick answer.
const DIFF_MAX: usize = 60_000;

/// The kept change as a diff, file by file: each file's text in HEAD against what the commit takes.
pub fn kept_diff(files: &[(Option<String>, Kept)]) -> String {
    let mut out = String::new();
    for (head, kept) in files {
        let merged = Merged::diff(head.as_deref().unwrap_or(""), kept.text.as_deref().unwrap_or(""));
        out.push_str(&format!("--- {path}\n+++ {path}\n", path = kept.path));
        let rows: Vec<&str> = merged.text().split_inclusive('\n').collect();
        for hunk in merged.hunks() {
            for row in hunk.removed.clone() {
                out.push_str(&format!("-{}", rows.get(row).unwrap_or(&"\n")));
            }
            for row in hunk.added.clone() {
                out.push_str(&format!("+{}", rows.get(row).unwrap_or(&"\n")));
            }
        }
        if out.len() > DIFF_MAX {
            out.truncate(out.char_indices().map(|(i, _)| i).take_while(|i| *i <= DIFF_MAX).last().unwrap_or(0));
            out.push_str("\n[the rest of the diff is left out]\n");
            break;
        }
    }
    out
}

pub fn commit_prompt(diff: &str) -> String {
    format!(
        "Write a git commit message for this change. The first line says what it does in the imperative, at most 72 characters; then a blank line and a short body only if the change needs one. Answer with the message only.\n\n{diff}"
    )
}

pub fn branch_prompt(diff: &str) -> String {
    format!(
        "Name a git branch for this change: two to five lowercase words joined by hyphens, with a prefix such as fix/ or feat/. Answer with the branch name only.\n\n{diff}"
    )
}

/// The agent's answer as a message: no code fences, no surrounding quotes.
pub fn message(draft: &str) -> String {
    // The trailers are the reader's to add: an agent adds its own out of habit, with a name it guesses.
    let trailer = |l: &str| {
        let l = l.trim_start().to_ascii_lowercase();
        l.starts_with("co-authored-by:") || l.starts_with("signed-off-by:")
    };
    let lines: Vec<&str> = draft.trim().lines().filter(|l| !l.trim_start().starts_with("```") && !trailer(l)).collect();
    let text = lines.join("\n");
    text.trim().trim_matches(|c| c == '"' || c == '`' || c == '\'').trim().to_string()
}

/// The message with a line that names the task it works on, unless it names it already. The reader sees
/// the line in the draft and can take it out.
pub fn with_refs(message: &str, key: &str) -> String {
    let line = format!("Refs {key}");
    if message.lines().any(|l| l.trim() == line) {
        return message.to_string();
    }
    format!("{}\n\n{line}", message.trim_end())
}
/// The agent's answer as a branch name git takes: its first line, lowercase, hyphens for anything else.
pub fn branch_name(draft: &str) -> Option<String> {
    let line = draft.trim().lines().next()?.trim().trim_matches(|c| c == '`' || c == '"' || c == '\'');
    let mut name = String::new();
    for c in line.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '_' { c } else { '-' };
        if !(c == '-' && (name.is_empty() || name.ends_with('-'))) {
            name.push(c);
        }
    }
    let name: String = name.trim_matches(|c| c == '-' || c == '/' || c == '.').chars().take(60).collect();
    let name = name.trim_end_matches(['-', '/', '.']).replace("..", ".");
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests;
