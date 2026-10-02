use atelier_forge::{Side, Thread};

use crate::diff::FileView;
use super::structs::Placement;

pub fn place(threads: &[Thread], view: &FileView) -> Placement {
    let mut placement = Placement::default();
    let Some(shown) = view.shown() else {
        // A file with no rows still has its threads: they list above.
        for (i, t) in threads.iter().enumerate().filter(|(_, t)| belongs(t, view)) {
            if t.file_level { placement.file_level.push(i) } else { placement.outdated.push(i) }
        }
        return placement;
    };
    for (i, thread) in threads.iter().enumerate().filter(|(_, t)| belongs(t, view)) {
        if thread.file_level {
            placement.file_level.push(i);
            continue;
        }
        let row = match (thread.outdated, thread.line, thread.side) {
            (false, Some(line), Side::Right) => shown.lines.head_row(line),
            (false, Some(line), Side::Left) => shown.lines.base_row(line),
            _ => None,
        };
        match row {
            Some(row) => placement.rows.push((row, i)),
            None => placement.outdated.push(i),
        }
    }
    placement.rows.sort();
    placement
}

fn belongs(thread: &Thread, view: &FileView) -> bool {
    thread.path == view.path || view.old_path.as_deref() == Some(thread.path.as_str())
}
