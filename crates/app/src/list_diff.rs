//! Which rows of a session's list changed after a frame's events, so the virtual list measures only
//! those again and keeps its scroll. Each item has a small fingerprint of what its row draws; two lists
//! of fingerprints give the ranges to replace. Pure.

use std::ops::Range;

use lathe_agents::session::{Answer, Item, SubagentStatus, ToolStatus};

/// What a row draws, in a few numbers: equal fingerprints draw the same row.
pub fn fingerprint(item: &Item) -> (u8, usize, usize) {
    let tool = |status: ToolStatus| match status {
        ToolStatus::Pending => 0,
        ToolStatus::Running => 1,
        ToolStatus::Done => 2,
        ToolStatus::Failed => 3,
    };
    match item {
        Item::User { text } => (0, text.len(), 0),
        Item::Text { text, .. } => (1, text.len(), 0),
        Item::Thinking { text, took, .. } => (2, text.len(), usize::from(took.is_some())),
        Item::Tool(call) => (3, tool(call.call.status), call.output.as_ref().map_or(0, |o| o.text.len() + 1)),
        Item::Subagent { status, activity, calls, summary, .. } => {
            let status = match status {
                SubagentStatus::Running => 0,
                SubagentStatus::Done => 1,
                SubagentStatus::Failed => 2,
            };
            let done = calls.iter().filter(|c| c.output.is_some()).count();
            (4, status * 1_000_000 + calls.len() * 1000 + done, activity.as_ref().map_or(0, String::len) + summary.as_ref().map_or(0, String::len))
        }
        Item::Permission { answer, .. } => (5, match answer {
            Answer::Asking => 0,
            Answer::Answered(kind) => 1 + *kind as usize,
            Answer::Withdrawn => 9,
        }, 0),
        Item::Notice(text) => (6, text.len(), 0),
    }
}

/// One row of a session's list: an item of the conversation, or the files a finished turn changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Item(usize),
    Changes { turn: usize },
}

/// The rows for `items` conversation items, with each turn's changed files after the item it ended at:
/// `marks` holds `(items before the card, turn)`, in order.
pub fn rows(items: usize, marks: &[(usize, usize)]) -> Vec<Row> {
    let mut out = Vec::with_capacity(items + marks.len());
    let mut marks = marks.iter().peekable();
    for ix in 0..=items {
        while let Some(&&(at, turn)) = marks.peek()
            && at <= ix
        {
            out.push(Row::Changes { turn });
            marks.next();
        }
        if ix < items {
            out.push(Row::Item(ix));
        }
    }
    out
}

/// A turn's card draws the same files once the turn is kept: its turn is its fingerprint.
pub fn changes_fingerprint(turn: usize) -> (u8, usize, usize) {
    (7, turn, 0)
}

/// The replacements that turn a list of `before` rows into `after`: `(old range, new count)`, in order
/// from the end, so each applies without moving the next.
pub fn changes(before: &[(u8, usize, usize)], after: &[(u8, usize, usize)]) -> Vec<(Range<usize>, usize)> {
    let common = before.len().min(after.len());
    let mut out = Vec::new();
    if after.len() != before.len() {
        out.push((common..before.len(), after.len() - common));
    }
    let mut i = common;
    while i > 0 {
        i -= 1;
        if before[i] != after[i] {
            let end = i + 1;
            while i > 0 && before[i - 1] != after[i - 1] {
                i -= 1;
            }
            out.push((i..end, end - i));
        }
    }
    out
}

#[cfg(test)]
mod tests;
