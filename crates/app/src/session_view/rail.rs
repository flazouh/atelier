//! What the conversation's rail holds, from the conversation: one entry for each message the reader sent, with the start
//! of that message and the start of what came back, and the row of the list each one is at. Pure.
use beui::message_rail::{DESCRIPTION_CHARS, LABEL_CHARS, RailItem, excerpt};
use lathe_agents::session::Item;

use crate::list_diff::Row;

/// The entries, in order, each with the row of the list that shows its message.
pub fn entries(items: &[Item], shown: &[Row]) -> Vec<(RailItem, usize)> {
    let row_of = |item: usize| shown.iter().position(|r| *r == Row::Item(item));
    items
        .iter()
        .enumerate()
        .filter_map(|(ix, item)| {
            let Item::User { text } = item else { return None };
            let row = row_of(ix)?;
            // What came back: the first text the agent wrote before the next message of the reader's.
            let answer = items[ix + 1..].iter().take_while(|i| !matches!(i, Item::User { .. })).find_map(|i| match i {
                Item::Text { text, .. } => Some(excerpt(text, DESCRIPTION_CHARS)),
                _ => None,
            });
            Some((RailItem { label: excerpt(text, LABEL_CHARS).into(), description: answer.filter(|a| !a.is_empty()).map(Into::into) }, row))
        })
        .collect()
}

/// Which entry is in view: the last whose row is at or above the list's top row; the last of all while the list follows the
/// output; the first when the top is above them all.
pub fn active(rows: &[usize], top_row: usize, following: bool) -> usize {
    if following || rows.is_empty() {
        return rows.len().saturating_sub(1);
    }
    rows.iter().rposition(|&r| r <= top_row).unwrap_or(0)
}

#[cfg(test)]
mod tests;
