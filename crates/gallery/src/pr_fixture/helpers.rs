use std::{
    path::{Path},
};

use atelier_ui::{InlineHunk, RowMap};

use super::structs::{Change, Shown};

pub(super) fn shown(change: &Change) -> Shown {
    let head: Vec<&str> = change.head.trim_end_matches('\n').split('\n').collect();
    let mut rows: Vec<&str> = Vec::new();
    let mut hunks = Vec::new();
    let mut next = 0;
    for (n, &(at, removed, added)) in change.hunks.iter().enumerate() {
        rows.extend(&head[next..at]);
        let removed_rows = rows.len()..rows.len() + removed.len();
        rows.extend(removed);
        let added_rows = rows.len()..rows.len() + added;
        rows.extend(&head[at..at + added]);
        next = at + added;
        hunks.push(InlineHunk::new(format!("{}-{n}", change.path), removed_rows, added_rows));
    }
    rows.extend(&head[next..]);
    let rows_map = RowMap::new(&hunks);
    Shown { path: change.path, text: rows.join("\n"), hunks, rows: rows_map }
}

/// Every file in the repository at `root`, relative and sorted, as Go to file offers them: what
/// `.gitignore` leaves, hidden files left out. It reads the disk, so call it off the UI thread.
pub fn list_files(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|entry| entry.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}
