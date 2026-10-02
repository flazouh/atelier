use super::types::{ANSWER_MOST, TITLE_MOST};

/// The prompt for the title: what the reader asked, and the start of what the agent answered.
pub fn prompt(asked: &str, answered: &str) -> String {
    let answered: String = answered.chars().take(ANSWER_MOST).collect();
    format!(
        "In 3 to 6 words, say what the reader asked for in this coding session, as a commit subject does: \
         sentence case, starting with a verb, no quotes, no full stop. For example: Fix the flaky lease test. \
         Answer with those words alone.\n\nThe reader asked:\n{asked}\n\nThe agent answered:\n{answered}"
    )
}

/// The draft as a title: its first line, with no quotes, heading mark, "Title:" or full stop, cut to
/// [`TITLE_MOST`] characters at a word; `None` when nothing is left.
pub fn clean(draft: &str) -> Option<String> {
    let line = draft.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = line.trim_start_matches('#').trim();
    let line = line.strip_prefix("Title:").unwrap_or(line).trim();
    let line = line.trim_matches(|c| c == '"' || c == '\'' || c == '`').trim().trim_end_matches('.').trim();
    if line.is_empty() {
        return None;
    }
    if line.chars().count() <= TITLE_MOST {
        return Some(line.to_string());
    }
    let cut: String = line.chars().take(TITLE_MOST).collect();
    let at_word = cut.rfind(' ').map_or(cut.as_str(), |at| &cut[..at]);
    Some(at_word.trim_end().to_string())
}
