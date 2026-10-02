//! The text of an edit, told while its input still streams in.
use serde_json::{Value, json};

use super::replay;
use crate::{
    claude_code::tools::partial_input,
    session::{Event, ToolId},
};

#[test]
fn what_has_arrived_of_each_text_is_read_whole_or_cut() {
    for (json, want) in [
        (r#"{"file_path":"/w/a.rs","old_string":"fn a","new_string":"fn b() {\n    "#, Some(json!({"file_path": "/w/a.rs", "old_string": "fn a", "new_string": "fn b() {\n    "}))),
        (r#"{"file_path":"/w/a.rs","old_string":"x","new_string":"y"}"#, Some(json!({"file_path": "/w/a.rs", "old_string": "x", "new_string": "y"}))),
        (r#"{"file_path":"/w/a.rs","content":"say \"hi\" \"#, Some(json!({"file_path": "/w/a.rs", "content": "say \"hi\" "}))),
        (r#"{"content":"a\\"#, Some(json!({"content": "a\\"}))),
        (r#"{"content":"caf\u00e"#, Some(json!({"content": "caf"}))),
        (r#"{"content":"caf\u00e9 \ud83d"#, Some(json!({"content": "caf\u{e9} "}))),
        (r#"{"content":"\ud83d\ude00!"#, Some(json!({"content": "\u{1f600}!"}))),
        (r#"{"file_path":"/w/a.rs","replace_all":false,"new_string":"z"#, Some(json!({"file_path": "/w/a.rs", "new_string": "z"}))),
        (r#"{"file_p"#, None),
        (r#"{"file_path":"#, None),
        (r#"{"file_path":""#, Some(json!({"file_path": ""}))),
        ("", None),
    ] {
        assert_eq!(partial_input(json), want, "{json}");
    }
}

#[test]
fn the_same_words_inside_a_string_are_not_a_key() {
    let got = partial_input(r#"{"content":"\"new_string\": \"no\"","new_string":"yes"#).unwrap();
    assert_eq!(got, json!({"content": "\"new_string\": \"no\"", "new_string": "yes"}));
}

#[test]
fn a_captured_edit_tells_its_text_as_it_streams_and_ends_with_the_whole_input() {
    let events = replay("edit_file");
    let id = ToolId::new("toolu_01Jzq7Cn6sNgGBDwV6wr2wns");
    let inputs: Vec<&Value> = events.iter().filter_map(|e| if let Event::ToolInput { id: at, input, .. } = e { (*at == id).then_some(input) } else { None }).collect();
    assert!(inputs.len() >= 3, "text arrives in parts, then the whole input: {inputs:#?}");
    let new_len = |v: &&Value| v.get("new_string").and_then(Value::as_str).map_or(0, str::len);
    assert!(inputs.windows(2).all(|w| new_len(&w[0]) <= new_len(&w[1])), "a text never shrinks: {inputs:#?}");
    assert!(inputs.iter().any(|v| v.get("old_string").is_some() && v.get("new_string").is_none()), "the old text comes first");
    let last = inputs.last().unwrap();
    assert_eq!(last["new_string"], "goodbye world");
    assert_eq!(last["replace_all"], false, "the last input is the whole one");
}

#[test]
fn a_tool_that_writes_no_file_streams_no_partial_input() {
    let events = replay("tool_read");
    let partials = events.iter().filter(|e| matches!(e, Event::ToolInput { .. })).count();
    let calls = events.iter().filter(|e| matches!(e, Event::ToolStarted(_))).count();
    assert!(partials <= calls, "one whole input for each call at most: {partials} for {calls}");
}
