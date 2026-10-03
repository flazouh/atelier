//! The file a call will touch, told while its input still streams in.
use std::time::Instant;

use super::{fixture, replay};
use crate::{
    claude_code::{ClaudeLineMapper, LineMapper, tools::file_in_partial_input},
    session::{Conversation, Event, ToolKind},
};

#[test]
fn a_file_is_found_once_its_value_has_closed_and_not_before() {
    for (json, file) in [
        (r#"{"file_path":"/w/a.txt","content":"hi"}"#, Some("/w/a.txt")),
        (r#"{"file_path":"/w/a.txt"#, None),
        (r#"{"file_path"#, None),
        (r#"{"file_path":"#, None),
        (r#"{"file_path":"/w/a.txt""#, Some("/w/a.txt")),
        (r#"{ "file_path" : "/w/a b.txt" , "x": 1}"#, Some("/w/a b.txt")),
        (r#"{"notebook_path":"/w/n.ipynb"}"#, Some("/w/n.ipynb")),
        (r#"{"path":"/w/dir"}"#, Some("/w/dir")),
        (r#"{"file_path":"/w/q\"uote\\.txt"}"#, Some("/w/q\"uote\\.txt")),
        (r#"{"file_path":"/w/\u00e9.txt"}"#, Some("/w/\u{e9}.txt")),
        ("", None),
        (r#"{"command":"ls"}"#, None),
    ] {
        assert_eq!(file_in_partial_input(json).as_deref(), file, "{json}");
    }
}

#[test]
fn the_same_words_inside_a_string_or_a_nested_object_are_not_the_files_key() {
    assert_eq!(file_in_partial_input(r#"{"content":"\"file_path\": \"/etc/passwd\"","file_path":"/w/real.txt"}"#).as_deref(), Some("/w/real.txt"));
    assert_eq!(file_in_partial_input(r#"{"edits":[{"file_path":"/w/inner.txt"}]}"#), None);
    assert_eq!(file_in_partial_input(r#"{"content":"file_path"}"#), None);
}

#[test]
fn a_captured_write_names_its_file_before_the_whole_input_arrives() {
    let events = replay("permission_allow");
    let target = events.iter().position(|e| matches!(e, Event::ToolTarget { file, .. } if file.ends_with("/made.txt")));
    let input = events.iter().position(|e| matches!(e, Event::ToolInput { .. }));
    let ask = events.iter().position(|e| matches!(e, Event::Permission(_)));
    assert!(target.is_some(), "{events:#?}");
    assert!(target < input && target < ask, "the file is named before the input and before the question");
    assert_eq!(events.iter().filter(|e| matches!(e, Event::ToolTarget { .. })).count(), 1, "once per call");
}

#[test]
fn the_conversation_shows_the_file_from_the_target() {
    let mut conversation = Conversation::new();
    let mut mapper = ClaudeLineMapper::new();
    let start = Instant::now();
    for line in fixture("permission_allow").lines() {
        for event in mapper.line(line, start) {
            conversation.apply(&event);
            if matches!(event, Event::ToolTarget { .. }) {
                let call = conversation.items().iter().find_map(|i| if let crate::session::Item::Tool(c) = i { Some(c) } else { None }).unwrap();
                assert!(call.call.file.as_deref().is_some_and(|f| f.ends_with("/made.txt")));
                assert_eq!(call.call.kind, ToolKind::Write);
                return;
            }
        }
    }
    panic!("no target");
}

#[test]
fn a_call_that_names_no_file_gets_no_target() {
    let events = replay("tool_read");
    assert!(events.iter().all(|e| !matches!(e, Event::ToolTarget { .. })), "a shell command names no file");
}
