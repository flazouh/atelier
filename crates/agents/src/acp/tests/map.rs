//! Session updates to events, on a session the agent has started and a turn that runs.
use std::time::Duration;

use serde_json::json;

use super::{Run, chunk, update};
use crate::session::{
    BlockId, Command, Event, FileEdit, TodoStatus, ToolId, ToolKind, ToolOutput, ToolStatus, TurnOutcome,
};

/// A session with a turn running: atelier's prompt is request 2.
fn turn() -> Run {
    let mut run = Run::ready();
    run.command(Command::send("go"));
    run.events();
    run
}

fn tool_call(fields: serde_json::Value) -> serde_json::Value {
    let mut body = json!({ "sessionUpdate": "tool_call" });
    body.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    update(body)
}

fn tool_update(fields: serde_json::Value) -> serde_json::Value {
    let mut body = json!({ "sessionUpdate": "tool_call_update" });
    body.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    update(body)
}

#[test]
fn message_chunks_stream_into_one_block_until_something_else_comes() {
    let mut run = turn();
    run.agent(chunk("agent_message_chunk", "Hel")).agent(chunk("agent_message_chunk", "lo")).agent(chunk("agent_message_chunk", ""));
    assert_eq!(run.events(), vec![
        Event::Text { block: BlockId(1), delta: "Hel".into() },
        Event::Text { block: BlockId(1), delta: "lo".into() },
    ]);
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "ls", "kind": "execute" })));
    run.agent(chunk("agent_message_chunk", "Done"));
    assert!(run.events().contains(&Event::Text { block: BlockId(2), delta: "Done".into() }), "a new block after the call");
}

#[test]
fn a_thought_opens_a_thinking_block_that_ends_with_its_time() {
    let mut run = turn();
    run.agent(chunk("agent_thought_chunk", "")).agent(chunk("agent_thought_chunk", "plan it")).agent(chunk("agent_message_chunk", "ok"));
    assert_eq!(run.events(), vec![
        Event::Thinking { block: BlockId(1), delta: String::new() },
        Event::Thinking { block: BlockId(1), delta: "plan it".into() },
        // Opened at the first chunk, closed by the text two lines later.
        Event::ThinkingDone { block: BlockId(1), took: Duration::from_millis(2) },
        Event::Text { block: BlockId(2), delta: "ok".into() },
    ]);
}

#[test]
fn a_tool_call_starts_runs_takes_its_input_and_finishes_with_its_text() {
    let mut run = turn();
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Read file", "kind": "read", "status": "pending", "locations": [{ "path": "/work/project/src/a.rs" }] })));
    let Event::ToolStarted(call) = &run.events()[0] else { panic!("the call starts") };
    assert_eq!((call.id.as_str(), call.name.as_str(), call.kind, call.status), ("t1", "Read file", ToolKind::Read, ToolStatus::Pending));
    assert_eq!(call.file.as_deref(), Some("/work/project/src/a.rs"));
    assert!(call.input.is_null());

    run.agent(tool_update(json!({ "toolCallId": "t1", "status": "in_progress", "rawInput": { "path": "src/a.rs" } })));
    assert_eq!(run.events(), vec![
        Event::ToolInput { id: ToolId::new("t1"), input: json!({ "path": "src/a.rs" }), file: Some("/work/project/src/a.rs".into()) },
        Event::ToolStatus { id: ToolId::new("t1"), status: ToolStatus::Running },
    ]);

    run.agent(tool_update(json!({ "toolCallId": "t1", "status": "completed", "content": [{ "type": "content", "content": { "type": "text", "text": "fn main() {}" } }] })));
    assert_eq!(run.events(), vec![Event::ToolFinished {
        id: ToolId::new("t1"),
        output: ToolOutput { text: "fn main() {}".into(), is_error: false, truncated: false, full_at: None },
    }]);
}

#[test]
fn a_file_named_after_the_start_is_a_target_so_a_review_takes_its_text_first() {
    let mut run = turn();
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Edit", "kind": "edit" })));
    run.events();
    run.agent(tool_update(json!({ "toolCallId": "t1", "locations": [{ "path": "/work/project/a.rs" }] })));
    assert_eq!(run.events(), vec![Event::ToolTarget { id: ToolId::new("t1"), file: "/work/project/a.rs".into() }]);
}

#[test]
fn a_diff_with_no_old_text_is_a_write_and_its_output_says_what_changed() {
    let mut run = turn();
    let diff = json!([{ "type": "diff", "path": "/work/project/new.rs", "oldText": null, "newText": "x" }]);
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Write new.rs", "kind": "edit", "status": "completed", "content": diff })));
    let events = run.events();
    let Event::ToolStarted(call) = &events[0] else { panic!("the call starts") };
    assert_eq!(call.kind, ToolKind::Write);
    assert_eq!(call.status, ToolStatus::Running, "a call announced as done still starts as running");
    assert_eq!(call.file.as_deref(), Some("/work/project/new.rs"), "a diff names the file");
    assert!(matches!(events.last(), Some(Event::ToolFinished { output, .. }) if output.text == "Created /work/project/new.rs"));

    let edit = json!([{ "type": "diff", "path": "/work/project/a.rs", "oldText": "a", "newText": "b" }]);
    run.agent(tool_call(json!({ "toolCallId": "t2", "title": "Edit a.rs", "kind": "edit", "status": "completed", "content": edit })));
    let events = run.events();
    assert!(matches!(&events[0], Event::ToolStarted(call) if call.kind == ToolKind::Edit));
    assert!(matches!(events.last(), Some(Event::ToolFinished { output, .. }) if output.text == "Changed /work/project/a.rs"));
}

/// A diff is the call's edit in the words every agent shares: a new file has no old text, Cursor's `-- /dev/null`
/// included, and the edit is told before the call ends, so it shows with the call.
#[test]
fn a_diff_is_told_as_the_calls_edit_before_it_ends() {
    let mut run = turn();
    let edit = json!([{ "type": "diff", "path": "/work/project/a.rs", "oldText": "a", "newText": "b" }]);
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Edit a.rs", "kind": "edit", "status": "completed", "content": edit })));
    let events = run.events();
    let told = events.iter().position(|e| matches!(e, Event::ToolEdit { id, edit } if id.as_str() == "t1" && *edit == FileEdit { path: "/work/project/a.rs".into(), old: "a".into(), new: "b".into() }));
    let finished = events.iter().position(|e| matches!(e, Event::ToolFinished { .. }));
    assert!(told.is_some() && told < finished, "{events:?}");

    run.agent(tool_call(json!({ "toolCallId": "t2", "title": "Edit File", "kind": "edit", "status": "pending" })));
    assert!(!run.events().iter().any(|e| matches!(e, Event::ToolEdit { .. })), "no diff, no edit");
    let created = json!([{ "type": "diff", "path": "/work/project/new.rs", "oldText": "-- /dev/null", "newText": "x" }]);
    run.agent(tool_update(json!({ "toolCallId": "t2", "status": "completed", "content": created })));
    let events = run.events();
    assert!(events.iter().any(|e| matches!(e, Event::ToolEdit { edit, .. } if *edit == FileEdit { path: "/work/project/new.rs".into(), old: String::new(), new: "x".into() })), "{events:?}");
}

#[test]
fn an_edit_whose_diff_comes_later_and_makes_a_new_file_turns_into_a_write() {
    let mut run = turn();
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Edit File", "kind": "edit", "status": "pending" })));
    assert!(matches!(&run.events()[0], Event::ToolStarted(call) if call.kind == ToolKind::Edit));
    let diff = json!([{ "type": "diff", "path": "/work/project/new.rs", "oldText": "-- /dev/null", "newText": "x" }]);
    run.agent(tool_update(json!({ "toolCallId": "t1", "status": "completed", "content": diff })));
    let events = run.events();
    let kind = events.iter().position(|e| matches!(e, Event::ToolKind { id, kind: ToolKind::Write } if id.as_str() == "t1"));
    let finished = events.iter().position(|e| matches!(e, Event::ToolFinished { .. }));
    assert!(kind.is_some() && kind < finished, "the write is known before the call ends: {events:?}");
}

#[test]
fn acp_kinds_map_to_ateliers() {
    let mut run = turn();
    for (i, (kind, want)) in [
        ("read", ToolKind::Read),
        ("edit", ToolKind::Edit),
        ("delete", ToolKind::Edit),
        ("move", ToolKind::Edit),
        ("search", ToolKind::Search),
        ("execute", ToolKind::Shell),
        ("fetch", ToolKind::Fetch),
        ("think", ToolKind::Other),
        ("something_new", ToolKind::Other),
    ]
    .into_iter()
    .enumerate()
    {
        run.agent(tool_call(json!({ "toolCallId": format!("t{i}"), "title": kind, "kind": kind })));
        let events = run.events();
        assert!(matches!(&events[0], Event::ToolStarted(call) if call.kind == want), "{kind}");
    }
}

#[test]
fn a_failed_call_finishes_as_an_error_with_its_raw_output_when_it_has_no_content() {
    let mut run = turn();
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "Shell", "kind": "execute", "status": "in_progress" })));
    run.events();
    run.agent(tool_update(json!({ "toolCallId": "t1", "status": "failed", "rawOutput": { "exitCode": 1 } })));
    assert_eq!(run.events(), vec![Event::ToolFinished {
        id: ToolId::new("t1"),
        output: ToolOutput { text: r#"{"exitCode":1}"#.into(), is_error: true, truncated: false, full_at: None },
    }]);
}

#[test]
fn a_raw_output_object_gives_its_text_not_its_json() {
    let mut run = turn();
    for (i, (raw, want)) in [
        (json!({ "content": "fn main() {}" }), "fn main() {}"),
        (json!({ "exitCode": 0, "stdout": "a.rs\n", "stderr": "" }), "a.rs\n"),
        (json!({ "exitCode": 2, "stdout": "", "stderr": "no such file" }), "no such file"),
        (json!({ "exitCode": 1, "stdout": "half", "stderr": "then failed" }), "half\nthen failed"),
        (json!({ "exitCode": 0, "stdout": "", "stderr": "" }), ""),
        (json!({ "exitCode": 0, "stdout": "" }), ""),
        (json!({ "output": "" }), ""),
        (json!({ "content": "" }), ""),
        (json!({ "text": "", "stderr": "" }), ""),
    ]
    .into_iter()
    .enumerate()
    {
        run.agent(tool_call(json!({ "toolCallId": format!("t{i}"), "title": "Shell", "kind": "execute", "status": "completed", "rawOutput": raw })));
        let events = run.events();
        assert!(matches!(&events[1], Event::ToolFinished { output, .. } if output.text == want), "{want}");
    }
}

#[test]
fn a_long_output_is_cut_to_its_head() {
    let mut run = turn();
    let long = "é".repeat(ToolOutput::MAX_TEXT);
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "cat", "kind": "execute", "status": "completed", "rawOutput": long })));
    let events = run.events();
    let Event::ToolFinished { output, .. } = &events[1] else { panic!("the call finishes") };
    assert!(output.truncated && output.text.len() <= ToolOutput::MAX_TEXT);
    assert!(output.text.chars().all(|c| c == 'é'), "cut on a character boundary");
}

#[test]
fn a_plan_is_the_whole_todo_list() {
    let mut run = turn();
    run.agent(update(json!({ "sessionUpdate": "plan", "entries": [
        { "content": "Read", "priority": "high", "status": "completed" },
        { "content": "Fix", "priority": "medium", "status": "in_progress" },
        { "content": "Test", "priority": "low", "status": "pending" },
    ] })));
    let Event::Todos(todos) = &run.events()[0] else { panic!("todos") };
    let seen: Vec<_> = todos.iter().map(|t| (t.id.as_str(), t.text.as_str(), t.status)).collect();
    assert_eq!(seen, [("0", "Read", TodoStatus::Done), ("1", "Fix", TodoStatus::InProgress), ("2", "Test", TodoStatus::Pending)]);
}

#[test]
fn the_end_of_a_turn_fails_a_call_still_open_and_carries_the_closing_text() {
    let mut run = turn();
    run.agent(chunk("agent_message_chunk", "Looking."));
    run.agent(tool_call(json!({ "toolCallId": "t1", "title": "ls", "kind": "execute", "status": "in_progress" })));
    run.agent(chunk("agent_message_chunk", "All good."));
    run.events();
    run.agent(super::respond(2, json!({ "stopReason": "end_turn", "usage": { "totalTokens": 30, "inputTokens": 20, "outputTokens": 10, "cachedReadTokens": 5 } })));
    let events = run.events();
    assert!(matches!(&events[0], Event::ToolFinished { id, output } if id.as_str() == "t1" && output.is_error));
    assert!(matches!(&events[1], Event::Usage(usage) if usage.input_tokens == 20 && usage.output_tokens == 10 && usage.cache_read_tokens == 5));
    assert!(matches!(&events[2], Event::TurnEnded(end) if end.outcome == TurnOutcome::Completed && end.summary.as_deref() == Some("All good.")));
}

#[test]
fn an_update_of_a_kind_atelier_does_not_know_gives_nothing() {
    let mut run = turn();
    run.agent(update(json!({ "sessionUpdate": "usage_update", "used": 10, "size": 100 })));
    run.agent(update(json!({ "sessionUpdate": "brand_new_thing", "x": 1 })));
    assert_eq!(run.events(), vec![]);
}
