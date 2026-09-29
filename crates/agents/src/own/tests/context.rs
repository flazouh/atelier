use serde_json::json;

use crate::own::{
    context::{Budget, compact, estimate},
    message::{Block, Message, Role},
};

fn result(id: &str, size: usize) -> Message {
    Message { role: Role::User, blocks: vec![Block::ToolResult { id: id.into(), content: "x".repeat(size), is_error: false }] }
}

fn call(id: &str, content_size: usize) -> Message {
    Message::assistant(vec![Block::ToolUse { id: id.into(), name: "write".into(), input: json!({"path": "a.rs", "content": "y".repeat(content_size)}) }])
}

/// A conversation of `pairs` calls and results, each result `size` bytes.
fn conversation(pairs: usize, size: usize) -> Vec<Message> {
    let mut messages = vec![Message::user("start")];
    for i in 0..pairs {
        messages.push(call(&format!("c{i}"), 10));
        messages.push(result(&format!("c{i}"), size));
    }
    messages
}

fn result_len(m: &Message) -> usize {
    match &m.blocks[0] {
        Block::ToolResult { content, .. } => content.len(),
        _ => panic!(),
    }
}

#[test]
fn a_conversation_under_the_budget_is_left_alone() {
    let mut messages = conversation(5, 4000);
    let before = messages.clone();
    let done = compact(&mut messages, &Budget { limit_tokens: 100_000, keep_recent: 4 });
    assert_eq!((done.shortened, done.saved_bytes), (0, 0));
    assert_eq!(messages, before);
}

#[test]
fn old_results_are_shortened_oldest_first_and_the_newest_messages_stay_whole() {
    let mut messages = conversation(20, 4000);
    let limit = estimate(&messages) / 2;
    let done = compact(&mut messages, &Budget { limit_tokens: limit, keep_recent: 6 });
    assert!(done.shortened > 0 && done.saved_bytes > 0);
    assert!(estimate(&messages) <= limit, "it fits again: {} > {limit}", estimate(&messages));
    // The oldest result went first, and it keeps its head and a note.
    let first = match &messages[2].blocks[0] {
        Block::ToolResult { content, .. } => content.clone(),
        _ => panic!(),
    };
    assert!(first.starts_with("[omitted to save space: 4000 bytes."), "{first}");
    // The newest are untouched.
    for m in messages.iter().rev().take(6).filter(|m| m.role == Role::User) {
        assert_eq!(result_len(m), 4000);
    }
    // Shape stays: every call still has its result.
    assert_eq!(messages.len(), 41);
}

#[test]
fn it_shortens_well_below_the_limit_so_the_next_turn_does_not_do_it_again() {
    let mut messages = conversation(40, 4000);
    let limit = estimate(&messages) * 9 / 10;
    compact(&mut messages, &Budget { limit_tokens: limit, keep_recent: 4 });
    assert!(estimate(&messages) <= limit * 8 / 10 + 200, "{} vs {}", estimate(&messages), limit * 8 / 10);
    let again = compact(&mut messages, &Budget { limit_tokens: limit, keep_recent: 4 });
    assert_eq!(again.shortened, 0, "nothing to do until it grows again");
}

#[test]
fn big_strings_in_old_tool_inputs_are_shortened_too() {
    let mut messages = vec![Message::user("go"), call("w", 20_000), result("w", 10)];
    messages.extend(conversation(3, 100));
    compact(&mut messages, &Budget { limit_tokens: 1000, keep_recent: 2 });
    let Block::ToolUse { input, .. } = &messages[1].blocks[0] else { panic!() };
    assert!(input["content"].as_str().unwrap().starts_with("[omitted to save space"), "{input}");
    assert_eq!(input["path"], "a.rs", "short values stay");
}

#[test]
fn small_results_and_already_shortened_ones_are_left_and_the_estimate_counts_all_blocks() {
    let mut messages = conversation(10, 300);
    let done = compact(&mut messages, &Budget { limit_tokens: 10, keep_recent: 1 });
    assert_eq!(done.shortened, 0, "300 bytes is not worth a note");
    let mut messages = conversation(10, 4000);
    compact(&mut messages, &Budget { limit_tokens: 10, keep_recent: 1 });
    let after = messages.clone();
    compact(&mut messages, &Budget { limit_tokens: 10, keep_recent: 1 });
    assert_eq!(messages, after, "a shortened result is not shortened again");
    let thinking = Message::assistant(vec![Block::Thinking { text: "t".repeat(400), signature: Some("s".repeat(400)) }, Block::Redacted { data: "d".repeat(400) }]);
    assert!(estimate(&[thinking]) >= 300);
}

#[test]
fn a_cut_never_splits_a_character() {
    let mut messages = vec![Message::user("x"), Message { role: Role::User, blocks: vec![Block::ToolResult { id: "1".into(), content: "é".repeat(2000), is_error: false }] }, Message::user("y")];
    compact(&mut messages, &Budget { limit_tokens: 10, keep_recent: 1 });
    assert!(result_len(&messages[1]) < 600);
}
