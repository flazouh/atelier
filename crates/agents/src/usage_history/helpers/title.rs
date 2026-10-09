use crate::usage_history::consts::TITLE_MAX_CHARS;

/// One line of at most [`TITLE_MAX_CHARS`] characters (an ellipsis marks a cut); empty when the text is blank.
pub(crate) fn clean_title(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= TITLE_MAX_CHARS {
        return one_line;
    }
    let cut: String = one_line.chars().take(TITLE_MAX_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// A prompt worth a title: not blank and not a system tag such as `<command-name>`.
pub(crate) fn is_prompt(text: &str) -> bool {
    let t = text.trim_start();
    !t.is_empty() && !t.starts_with('<')
}
