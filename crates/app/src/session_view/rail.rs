//! What the conversation's rail holds, from the conversation: one entry for each message the reader sent, with the start
//! of that message, and the row of the list each one is at. Pure.
use atelier_ui::message_rail::{LABEL_CHARS, RailItem, excerpt};
use atelier_agents::session::Item;

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
            Some((RailItem { label: excerpt(text, LABEL_CHARS).into(), description: None }, row))
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
