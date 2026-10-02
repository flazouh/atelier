use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput};

use crate::lines::RowTokens;
use super::structs::RowChange;
use super::types::MIN_SIMILARITY;

/// The byte ranges of `old` and of `new` that differ, in words: a run of letters, digits and
/// underscores is one word, a run of spaces is one, and any other character stands alone.
pub fn word_changes(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let (old_words, new_words) = (words(old), words(new));
    let old_text: Vec<&str> = old_words.iter().map(|r| &old[r.clone()]).collect();
    let new_text: Vec<&str> = new_words.iter().map(|r| &new[r.clone()]).collect();
    let input = InternedInput::new(RowTokens(&old_text), RowTokens(&new_text));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    for hunk in diff.hunks() {
        push(&mut removed, &old_words, hunk.before.start as usize..hunk.before.end as usize);
        push(&mut added, &new_words, hunk.after.start as usize..hunk.after.end as usize);
    }
    (removed, added)
}

fn push(out: &mut Vec<Range<usize>>, words: &[Range<usize>], span: Range<usize>) {
    if span.is_empty() {
        return;
    }
    let range = words[span.start].start..words[span.end - 1].end;
    match out.last_mut() {
        Some(last) if last.end == range.start => last.end = range.end,
        _ => out.push(range),
    }
}

/// The byte range of every word of `text`, in order, covering all of it.
pub(super) fn words(text: &str) -> Vec<Range<usize>> {
    #[derive(PartialEq)]
    enum Class {
        Word,
        Space,
        Other,
    }
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else if c.is_whitespace() {
            Class::Space
        } else {
            Class::Other
        }
    };
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut last: Option<Class> = None;
    for (at, c) in text.char_indices() {
        let now = class(c);
        match (&last, out.last_mut()) {
            (Some(before), Some(range)) if *before == now && now != Class::Other => range.end = at + c.len_utf8(),
            _ => out.push(at..at + c.len_utf8()),
        }
        last = Some(now);
    }
    out
}

/// How much of the two rows is the same: 1 for equal rows, 0 for rows with nothing in common.
fn similarity(old: &str, new: &str, removed: &[Range<usize>], added: &[Range<usize>]) -> f64 {
    let total = old.len() + new.len();
    if total == 0 {
        return 1.;
    }
    let changed: usize = removed.iter().chain(added).map(|r| r.len()).sum();
    1. - changed as f64 / total as f64
}

/// The rows of one hunk that are the same line changed, paired in order, each with its changed words.
/// A row with no partner is left out: it is a line the agent wrote or removed whole.
pub fn pair_rows(removed: &[&str], added: &[&str]) -> Vec<RowChange> {
    let mut pairs = Vec::new();
    let mut cursor = 0;
    for (i, old) in removed.iter().enumerate() {
        for (j, new) in added.iter().enumerate().skip(cursor) {
            let (r, a) = word_changes(old, new);
            if similarity(old, new, &r, &a) >= MIN_SIMILARITY {
                pairs.push(RowChange { removed_row: i, added_row: j, removed: r, added: a });
                cursor = j + 1;
                break;
            }
            // Rows are paired in order, and a hunk that replaces line for line has its partner at
            // once: do not look far for a partner of a row that has none.
            if j >= cursor + 2 {
                break;
            }
        }
    }
    pairs
}
