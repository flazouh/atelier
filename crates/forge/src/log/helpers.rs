/// The longest line a card shows; a longer one ends in `…`.
const MOST: usize = 200;

/// What GitHub writes under every failed step, which says nothing of why.
const EXIT_LINE: &str = "Process completed with exit code";

/// Words that start or mark a line that says what went wrong, lowercased.
const MARKS: [&str; 8] = ["panicked at", "assertionerror", "timed out", "exception", "fail:", "failed", "fatal:", "✗"];

/// The first line of `log` that says why the job failed, without its timestamp or colours: an annotation
/// GitHub was asked to show, else the first line that reads like an error, else GitHub's own exit line.
pub fn first_error_line(log: &str) -> Option<String> {
    let lines: Vec<String> = log.lines().map(clean).filter(|l| !l.is_empty()).collect();
    let annotation = lines.iter().filter_map(|l| l.strip_prefix("##[error]")).map(str::trim).find(|l| !l.starts_with(EXIT_LINE));
    let marked = || {
        lines.iter().filter(|l| !l.starts_with("##[")).map(String::as_str).find(|l| {
            let lower = l.to_lowercase();
            lower.starts_with("error") || MARKS.iter().any(|m| lower.contains(m))
        })
    };
    let exit = || lines.iter().find_map(|l| l.strip_prefix("##[error]")).map(str::trim);
    annotation.or_else(marked).or_else(exit).map(cut)
}

/// `line` without the timestamp GitHub puts first, nor terminal colour codes.
fn clean(line: &str) -> String {
    let line = match line.split_once(' ') {
        Some((stamp, rest)) if stamp.len() > 20 && stamp.ends_with('Z') && stamp.as_bytes()[4] == b'-' => rest,
        _ => line,
    };
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() == Some('[') {
                chars.by_ref().find(|c| ('@'..='~').contains(c));
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

fn cut(line: &str) -> String {
    if line.chars().count() <= MOST {
        return line.to_string();
    }
    let kept: String = line.chars().take(MOST - 1).collect();
    format!("{}…", kept.trim_end())
}
