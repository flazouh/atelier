//! The text of an edit, told while its input still streams in.
use super::replay;
use crate::session::{Event, FileEdit, ToolId};

#[test]
fn a_captured_edit_tells_its_text_as_it_streams_and_ends_with_the_whole_text() {
    let events = replay("edit_file");
    let id = ToolId::new("toolu_01Jzq7Cn6sNgGBDwV6wr2wns");
    let edits: Vec<&FileEdit> = events.iter().filter_map(|e| if let Event::ToolEdit { id: at, edit } = e { (*at == id).then_some(edit) } else { None }).collect();
    assert!(edits.len() >= 3, "the text arrives in parts, then whole: {edits:#?}");
    assert!(edits.windows(2).all(|w| w[0].new.len() <= w[1].new.len()), "a text never shrinks: {edits:#?}");
    assert!(edits.iter().any(|e| !e.old.is_empty() && e.new.is_empty()), "the old text comes first");
    assert_eq!(edits.last().unwrap().new, "goodbye world");
    assert!(edits.iter().all(|e| e.path.ends_with(".txt")), "{edits:#?}");
    let inputs = events.iter().filter(|e| matches!(e, Event::ToolInput { id: at, .. } if *at == id)).count();
    assert_eq!(inputs, 1, "the agent's own input is told once, whole");
}

#[test]
fn a_captured_write_ends_with_its_whole_text_and_no_old_text() {
    let events = replay("permission_allow");
    let last = events.iter().rev().find_map(|e| if let Event::ToolEdit { edit, .. } = e { Some(edit) } else { None }).expect("the write tells its text");
    assert!(last.path.ends_with("/made.txt"), "{last:?}");
    assert_eq!((last.old.as_str(), last.new.as_str()), ("", "hi"));
}

#[test]
fn a_tool_that_writes_no_file_tells_no_edit() {
    let events = replay("tool_read");
    assert!(!events.iter().any(|e| matches!(e, Event::ToolEdit { .. })));
}
