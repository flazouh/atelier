use std::time::{Duration, Instant};

use serde_json::json;

use super::{fixture, replay, replay_with};
use crate::{
    claude_code::Mapper,
    session::{
        ChoiceKind, EndReason, Event, RequestId, ToolId, ToolKind, ToolOutput, TodoStatus, TurnOutcome,
    },
};

fn text_of(events: &[Event]) -> String {
    events.iter().filter_map(|e| if let Event::Text { delta, .. } = e { Some(delta.as_str()) } else { None }).collect()
}

fn tool_named(events: &[Event], name: &str) -> crate::session::ToolCall {
    events
        .iter()
        .find_map(|e| match e {
            Event::ToolStarted(call) if call.name == name => Some(call.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {name} call"))
}

#[test]
fn a_plain_turn_streams_its_text_once_and_ends_completed() {
    let events = replay("plain");
    let Event::Started(started) = &events[0] else { panic!("the first event is {:?}", events[0]) };
    assert!(!started.session.as_str().is_empty());
    assert_eq!(text_of(&events), "ok", "the finished message must not repeat the streamed text");
    assert!(events.iter().all(|e| !matches!(e, Event::Warning(_))));
    assert!(events.iter().any(|e| matches!(e, Event::Usage(u) if u.output_tokens > 0 && u.cost_usd.is_some())));
    let Some(Event::TurnEnded(end)) = events.last() else { panic!("the last event is {:?}", events.last()) };
    assert_eq!(end.outcome, TurnOutcome::Completed);
    assert_eq!(end.summary.as_deref(), Some("ok"));
}

#[test]
fn thinking_streams_from_its_start_and_ends_with_its_time() {
    let events = replay("tool_read");
    let block = events.iter().find_map(|e| if let Event::Thinking { block, .. } = e { Some(*block) } else { None });
    let block = block.expect("the run thinks");
    let took = events.iter().find_map(|e| match e {
        Event::ThinkingDone { block: done, took } if *done == block => Some(*took),
        _ => None,
    });
    assert!(took.is_some_and(|t| t > Duration::ZERO), "thinking has a time: {took:?}");
}

#[test]
fn a_thinking_block_reports_the_time_between_its_start_and_its_stop() {
    let mut mapper = Mapper::new();
    let start = Instant::now();
    let start_line = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}}"#;
    let stop_line = r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0}}"#;
    mapper.line(start_line, start);
    let events = mapper.line(stop_line, start + Duration::from_millis(1500));
    assert!(matches!(events.as_slice(), [Event::ThinkingDone { took, .. }] if *took == Duration::from_millis(1500)));
}

#[test]
fn a_tool_call_starts_gets_its_input_and_finishes_with_its_output() {
    let events = replay("tool_read");
    let call = tool_named(&events, "Bash");
    assert_eq!(call.kind, ToolKind::Shell);
    let input = events.iter().find_map(|e| match e {
        Event::ToolInput { id, input, .. } if *id == call.id => Some(input.clone()),
        _ => None,
    });
    assert!(input.expect("the whole input arrives")["command"].as_str().unwrap().contains("note.txt"));
    let output = events.iter().find_map(|e| match e {
        Event::ToolFinished { id, output } if *id == call.id => Some(output.clone()),
        _ => None,
    });
    let output = output.expect("the call finishes");
    assert!(output.text.contains("hello") && !output.is_error);
}

#[test]
fn a_permission_request_names_the_tool_its_file_and_its_choices() {
    let mut mapper = Mapper::new();
    let events = replay_with(&mut mapper, &fixture("permission_allow"));
    let request = events
        .iter()
        .find_map(|e| if let Event::Permission(request) = e { Some(request.clone()) } else { None })
        .expect("a permission request");
    assert_eq!(request.call.name, "Write");
    assert_eq!(request.call.kind, ToolKind::Write);
    assert_eq!(request.call.file.as_deref(), Some("/work/made.txt"));
    assert_eq!(request.call.input["content"], "hi");
    let kinds: Vec<_> = request.choices.iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&ChoiceKind::Allow) && kinds.contains(&ChoiceKind::Deny));
}

#[test]
fn an_answer_is_written_once_and_only_for_a_request_that_waits() {
    let mut mapper = Mapper::new();
    let events = replay_with(&mut mapper, &fixture("permission_allow"));
    let Some(Event::Permission(request)) = events.iter().find(|e| matches!(e, Event::Permission(_))) else {
        panic!("no request")
    };
    let allow = request.choices.iter().find(|c| c.kind == ChoiceKind::Allow).unwrap().id.clone();
    // The captured run finished its turn, which withdraws what nobody answered.
    assert!(mapper.answer(&request.id, &allow).is_none());
    assert!(events.iter().any(|e| matches!(e, Event::PermissionCancelled(id) if *id == request.id)));
}

#[test]
fn allow_and_deny_answer_with_the_input_and_a_message() {
    let mut mapper = Mapper::new();
    let ask = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"},"tool_use_id":"t1"}}"#;
    let events = mapper.line(ask, Instant::now());
    let Event::Permission(request) = &events[0] else { panic!("no request") };
    assert_eq!(request.call.id, ToolId::new("t1"));
    let line = mapper.answer(&request.id, &request.choices[0].id).expect("the request waits");
    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(value["response"]["request_id"], "r1");
    assert_eq!(value["response"]["response"]["behavior"], "allow");
    assert_eq!(value["response"]["response"]["updatedInput"]["command"], "ls");

    let events = mapper.line(&ask.replace("r1", "r2"), Instant::now());
    let Event::Permission(request) = &events[0] else { panic!("no request") };
    let deny = request.choices.iter().find(|c| c.kind == ChoiceKind::Deny).unwrap();
    let value: serde_json::Value = serde_json::from_str(&mapper.answer(&request.id, &deny.id).unwrap()).unwrap();
    assert_eq!(value["response"]["response"]["behavior"], "deny");
    assert!(value["response"]["response"]["message"].as_str().is_some_and(|m| !m.is_empty()));
}

#[test]
fn always_allow_is_offered_only_with_rules_and_sends_them_back() {
    let mut mapper = Mapper::new();
    let rules = json!([{"type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "allow"}]);
    let ask = json!({"type": "control_request", "request_id": "r1", "request": {
        "subtype": "can_use_tool", "tool_name": "Bash", "input": {}, "permission_suggestions": rules}});
    let events = mapper.line(&ask.to_string(), Instant::now());
    let Event::Permission(request) = &events[0] else { panic!("no request") };
    let always = request.choices.iter().find(|c| c.kind == ChoiceKind::AllowAlways).expect("always allow");
    let value: serde_json::Value = serde_json::from_str(&mapper.answer(&request.id, &always.id).unwrap()).unwrap();
    assert_eq!(value["response"]["response"]["updatedPermissions"], rules);
}

#[test]
fn a_denied_tool_finishes_as_an_error_and_the_turn_still_completes() {
    let events = replay("permission_deny");
    let call = tool_named(&events, "Write");
    let output = events.iter().find_map(|e| match e {
        Event::ToolFinished { id, output } if *id == call.id => Some(output.clone()),
        _ => None,
    });
    assert!(output.expect("the call finishes").is_error);
    assert!(matches!(events.last(), Some(Event::TurnEnded(end)) if end.outcome == TurnOutcome::Completed));
}

#[test]
fn an_interrupt_during_a_tool_fails_the_tool_and_ends_the_turn_interrupted() {
    let events = replay("permission_interrupt");
    let call = tool_named(&events, "Write");
    let finished = events.iter().position(|e| matches!(e, Event::ToolFinished { id, output } if *id == call.id && output.is_error));
    let ended = events.iter().position(|e| matches!(e, Event::TurnEnded(end) if end.outcome == TurnOutcome::Interrupted));
    assert!(finished.is_some() && ended.is_some(), "{events:#?}");
    assert!(finished < ended, "the tool fails before the turn ends");
    assert!(events.iter().all(|e| !matches!(e, Event::UserMessage { .. })), "the interrupt note is not a user message");
}

#[test]
fn the_task_tools_build_the_todo_list() {
    let events = replay("task_list");
    let lists: Vec<_> = events.iter().filter_map(|e| if let Event::Todos(list) = e { Some(list.clone()) } else { None }).collect();
    let last = lists.last().expect("todo lists");
    let seen: Vec<_> = last.iter().map(|t| (t.text.as_str(), t.status)).collect();
    assert_eq!(seen, [("one", TodoStatus::Done), ("two", TodoStatus::InProgress)]);
    assert!(events.iter().all(|e| !matches!(e, Event::ToolStarted(c) if c.name.starts_with("Task"))));
}

#[test]
fn todo_write_replaces_the_whole_list() {
    let mut mapper = Mapper::new();
    let call = json!({"type": "assistant", "message": {"id": "m", "content": [{"type": "tool_use", "id": "t", "name": "TodoWrite",
        "input": {"todos": [{"content": "a", "status": "completed"}, {"content": "b", "status": "pending"}]}}]}});
    let events = mapper.line(&call.to_string(), Instant::now());
    let [Event::Todos(list)] = events.as_slice() else { panic!("{events:?}") };
    assert_eq!((list[0].status, list[1].status), (TodoStatus::Done, TodoStatus::Pending));
}

#[test]
fn a_foreground_subagent_starts_reports_progress_and_ends_with_its_calls_under_it() {
    let events = replay("subagent_foreground");
    let started = events.iter().find_map(|e| if let Event::SubagentStarted(s) = e { Some(s.clone()) } else { None }).expect("a subagent");
    assert_eq!(started.kind.as_deref(), Some("general-purpose"));
    assert!(started.task.contains("note.txt"));
    assert!(events.iter().any(|e| matches!(e, Event::SubagentProgress { id, .. } if *id == started.id)));
    let read = tool_named(&events, "Read");
    assert_eq!(read.parent.as_ref(), Some(&started.id));
    assert!(events.iter().any(|e| matches!(e, Event::SubagentEnded { id, ok: true, .. } if *id == started.id)));
    assert!(events.iter().all(|e| !matches!(e, Event::ToolStarted(c) if c.name == "Agent")));
    assert_eq!(events.iter().filter(|e| matches!(e, Event::SubagentStarted(_))).count(), 1);
}

#[test]
fn a_background_subagent_is_told_once_and_the_turn_can_end_before_it() {
    let events = replay("subagent_background");
    assert_eq!(events.iter().filter(|e| matches!(e, Event::SubagentStarted(_))).count(), 1);
    assert!(matches!(events.last(), Some(Event::TurnEnded(_))));
}

#[test]
fn a_saved_output_passes_on_its_preview_and_its_path() {
    let events = replay("long_output");
    let output: ToolOutput = events
        .iter()
        .find_map(|e| if let Event::ToolFinished { output, .. } = e { Some(output.clone()) } else { None })
        .expect("the call finishes");
    assert!(output.truncated);
    assert!(output.full_at.as_deref().is_some_and(|p| p.contains(".claude/projects/")));
    assert!(output.text.starts_with("1\n2\n3") && !output.text.contains("persisted-output"));
}

#[test]
fn a_ten_megabyte_result_keeps_a_head_and_says_it_was_cut() {
    let mut mapper = Mapper::new();
    let call = json!({"type": "assistant", "message": {"id": "m", "content": [{"type": "tool_use", "id": "t", "name": "Read", "input": {}}]}});
    mapper.line(&call.to_string(), Instant::now());
    let big = "é".repeat(5 * 1024 * 1024);
    let result = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t", "content": big}]}});
    let events = mapper.line(&result.to_string(), Instant::now());
    let [Event::ToolFinished { output, .. }] = events.as_slice() else { panic!("{}", events.len()) };
    assert!(output.truncated && output.text.len() <= ToolOutput::MAX_TEXT);
    assert!(output.text.chars().all(|c| c == 'é'), "the cut is on a character boundary");
}

#[test]
fn a_line_that_is_not_json_or_is_cut_short_is_a_warning_and_the_stream_goes_on() {
    let mut mapper = Mapper::new();
    for bad in ["not json at all", "{", r#"{"type":"assistant","message":"#, r#"{"type":"assistant","message":5}"#] {
        let events = mapper.line(bad, Instant::now());
        assert!(matches!(events.as_slice(), [Event::Warning(_)]), "{bad}: {events:?}");
    }
    let after = mapper.line(r#"{"type":"system","subtype":"init","session_id":"s"}"#, Instant::now());
    assert!(matches!(after.as_slice(), [Event::Started(_)]));
}

#[test]
fn blank_lines_and_kinds_atelier_does_not_know_give_nothing() {
    let mut mapper = Mapper::new();
    for line in ["", "   ", r#"{"type":"rate_limit_event"}"#, r#"{"type":"something_new","x":1}"#, r#"{"type":"system","subtype":"status"}"#] {
        assert!(mapper.line(line, Instant::now()).is_empty(), "{line}");
    }
}

#[test]
fn a_crash_mid_turn_fails_the_open_tool_and_the_turn_then_ends_the_session() {
    let mut mapper = Mapper::new();
    mapper.user_sent();
    let start = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t9","name":"Bash","input":{}}}}"#;
    assert!(matches!(mapper.line(start, Instant::now()).as_slice(), [Event::ToolStarted(_)]));
    let events = mapper.exited(Some(3), "");
    let [Event::ToolFinished { id, output }, Event::TurnEnded(end), Event::Ended(reason)] = events.as_slice() else {
        panic!("{events:#?}")
    };
    assert_eq!(id.as_str(), "t9");
    assert!(output.is_error && output.text.contains("code 3"));
    assert!(matches!(&end.outcome, TurnOutcome::Failed(why) if why.contains("code 3")));
    assert_eq!(*reason, EndReason::Exited { code: Some(3), stderr: String::new() });
}

#[test]
fn a_crash_with_no_turn_open_only_ends_the_session() {
    assert_eq!(Mapper::new().exited(None, ""), [Event::Ended(EndReason::Exited { code: None, stderr: String::new() })]);
}

#[test]
fn a_session_atelier_closed_ends_closed_and_fails_nothing() {
    let mut mapper = Mapper::new();
    mapper.user_sent();
    assert_eq!(mapper.closed(), [Event::Ended(EndReason::Closed)]);
}

#[test]
fn a_session_ends_once() {
    let mut mapper = Mapper::new();
    assert_eq!(mapper.exited(Some(0), "").len(), 1);
    assert!(mapper.closed().is_empty());
    assert!(mapper.exited(None, "").is_empty());
}

#[test]
fn a_permission_question_left_open_by_a_crash_is_cancelled() {
    let mut mapper = Mapper::new();
    let ask = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{}}}"#;
    mapper.line(ask, Instant::now());
    let events = mapper.exited(Some(1), "");
    assert!(events.contains(&Event::PermissionCancelled(RequestId::new("r1"))));
}

#[test]
fn a_transcript_shows_history_with_no_streaming() {
    let transcript = [
        json!({"type": "summary", "summary": "x"}),
        json!({"type": "user", "message": {"role": "user", "content": "fix the bug"}}),
        json!({"type": "assistant", "message": {"id": "m1", "content": [
            {"type": "text", "text": "Looking."}, {"type": "tool_use", "id": "t1", "name": "Read", "input": {"file_path": "/a.rs"}}]}}),
        json!({"type": "user", "toolUseResult": {"x": 1}, "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "fn main() {}"}]}}),
        json!({"type": "user", "isMeta": true, "message": {"role": "user", "content": "a hidden note"}}),
        json!({"type": "assistant", "isSidechain": true, "message": {"id": "m2", "content": [{"type": "text", "text": "subagent chatter"}]}}),
        json!({"type": "assistant", "message": {"id": "m3", "content": [{"type": "text", "text": "Done."}]}}),
    ]
    .map(|line| line.to_string())
    .join("\n");
    let events = crate::claude_code::history(&transcript);
    assert!(matches!(&events[0], Event::UserMessage { text } if text == "fix the bug"));
    assert_eq!(text_of(&events), "Looking.Done.");
    let call = tool_named(&events, "Read");
    assert_eq!(call.file.as_deref(), Some("/a.rs"));
    assert!(events.iter().any(|e| matches!(e, Event::ToolFinished { id, output } if *id == call.id && output.text == "fn main() {}")));
    assert_eq!(events.len(), 5, "{events:#?}");
}

#[test]
fn every_captured_run_folds_into_a_finished_conversation() {
    use crate::session::{Conversation, Item, SubagentStatus};
    for name in [
        "plain", "tool_read", "permission_allow", "permission_deny", "permission_interrupt", "task_list",
        "subagent_background", "subagent_foreground", "long_output",
    ] {
        let mut conversation = Conversation::new();
        conversation.user_sent("go");
        replay(name).iter().for_each(|event| conversation.apply(event));
        assert!(!conversation.working(), "{name}: the turn is over");
        assert!(conversation.started().is_some(), "{name}");
        assert!(
            conversation.items().iter().all(|item| !matches!(item, Item::Notice(_))),
            "{name}: {:?}",
            conversation.items()
        );
        assert!(
            conversation.items().iter().all(|item| !matches!(item, Item::Permission { answer: crate::session::Answer::Asking, .. })),
            "{name}: no question is left open"
        );
        assert!(conversation.items().iter().all(|item| !matches!(item, Item::Subagent { status: SubagentStatus::Failed, .. })), "{name}");
    }
}

#[test]
fn the_foreground_subagent_run_folds_with_its_call_inside_the_subagent() {
    use crate::session::{Conversation, Item};
    let mut conversation = Conversation::new();
    replay("subagent_foreground").iter().for_each(|event| conversation.apply(event));
    let inside = conversation.items().iter().find_map(|item| match item {
        Item::Subagent { calls, .. } => Some(calls.iter().map(|c| c.call.name.clone()).collect::<Vec<_>>()),
        _ => None,
    });
    assert_eq!(inside, Some(vec!["Read".to_string()]));
    assert!(conversation.items().iter().all(|item| !matches!(item, Item::Tool(c) if c.call.name == "Read")));
}

#[test]
fn an_early_exit_carries_the_last_lines_of_stderr_and_puts_the_last_one_in_the_failure() {
    let mut mapper = Mapper::new();
    mapper.user_sent();
    let stderr = "starting\nwarming up\nError: no such model\n\n";
    let events = mapper.exited(Some(3), stderr);
    let Some(Event::TurnEnded(end)) = events.iter().find(|e| matches!(e, Event::TurnEnded(_))) else { panic!("{events:#?}") };
    assert_eq!(end.outcome, TurnOutcome::Failed("the agent exited with code 3: Error: no such model".into()));
    assert_eq!(
        events.last(),
        Some(&Event::Ended(EndReason::Exited { code: Some(3), stderr: "starting\nwarming up\nError: no such model".into() }))
    );
}

#[test]
fn only_the_last_twenty_lines_of_a_long_stderr_are_kept() {
    let stderr: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    let events = Mapper::new().exited(Some(1), &stderr);
    let Some(Event::Ended(EndReason::Exited { stderr: kept, .. })) = events.last() else { panic!() };
    let lines: Vec<&str> = kept.lines().collect();
    assert_eq!((lines.len(), lines[0], lines[19]), (20, "line 31", "line 50"));
}

#[test]
fn a_signal_with_no_stderr_says_only_that() {
    let mut mapper = Mapper::new();
    mapper.user_sent();
    let events = mapper.exited(None, "  \n");
    assert!(events.iter().any(|e| matches!(e, Event::TurnEnded(end) if end.outcome == TurnOutcome::Failed("the agent was stopped by a signal".into()))));
}

/// A shell command run in the background is the call that started it, still running: `claude` reports it
/// with a `task_started` of type `local_bash`, and that is not a subagent. Captured with `claude` 2.1.284.
#[test]
fn a_background_shell_command_is_a_running_shell_call_not_a_subagent() {
    let events = replay("background_bash");
    assert!(!events.iter().any(|e| matches!(e, Event::SubagentStarted(_) | Event::SubagentEnded { .. })), "no subagent: {events:#?}");
    let call = tool_named(&events, "Bash");
    assert_eq!(call.kind, crate::session::ToolKind::Shell);
    let position = |pred: &dyn Fn(&Event) -> bool| events.iter().position(pred).unwrap_or_else(|| panic!("missing event in {events:#?}"));
    let started = position(&|e| matches!(e, Event::ToolStarted(c) if c.id == call.id));
    let finished = position(&|e| matches!(e, Event::ToolFinished { id, .. } if *id == call.id));
    // The result that only says "running in the background" is not the end of the call.
    let waiting = events[started..finished].iter().filter(|e| matches!(e, Event::TurnEnded(_))).count();
    assert_eq!(waiting, 1, "the first turn ended while the command still ran");
    let Event::ToolFinished { output, .. } = &events[finished] else { unreachable!() };
    assert!(!output.is_error);
    assert!(output.text.contains("completed (exit code 0)"), "{}", output.text);
    assert!(!output.text.contains("Command running in background"));
    assert_eq!(events.iter().filter(|e| matches!(e, Event::ToolFinished { id, .. } if *id == call.id)).count(), 1, "it finishes once");
}

#[test]
fn a_background_command_ends_as_failed_when_the_process_dies_first() {
    let mut mapper = Mapper::new();
    let now = Instant::now();
    let lines = fixture("background_bash");
    let up_to_start: Vec<&str> = lines.lines().take_while(|l| !l.contains("task_notification")).collect();
    let mut events: Vec<Event> = up_to_start.iter().flat_map(|l| mapper.line(l, now)).collect();
    events.extend(mapper.exited(Some(1), "boom"));
    let call = tool_named(&events, "Bash");
    assert!(events.iter().any(|e| matches!(e, Event::ToolFinished { id, output } if *id == call.id && output.is_error)), "{events:#?}");
}

fn ended_subagents(events: &[Event]) -> Vec<(String, bool, Option<String>)> {
    events.iter().filter_map(|e| if let Event::SubagentEnded { id, ok, summary } = e { Some((id.as_str().to_string(), *ok, summary.clone())) } else { None }).collect()
}

fn subagent_transcript(extra: &[serde_json::Value]) -> String {
    let mut lines = vec![
        json!({"type": "user", "message": {"role": "user", "content": "look around"}}),
        json!({"type": "assistant", "message": {"id": "m1", "content": [
            {"type": "tool_use", "id": "task1", "name": "Task", "input": {"description": "Count", "prompt": "Count to 60", "subagent_type": "general-purpose"}},
            {"type": "tool_use", "id": "sh1", "name": "Bash", "input": {"command": "sleep 60"}}]}}),
    ];
    lines.extend(extra.iter().cloned());
    lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")
}

/// A record has no end for a subagent or a call still going when the record stopped. In history they end.
#[test]
fn history_ends_every_subagent_and_call_the_record_left_open() {
    let events = crate::claude_code::history(&subagent_transcript(&[]));
    assert!(events.iter().any(|e| matches!(e, Event::SubagentStarted(s) if s.id.as_str() == "task1")), "{events:#?}");
    let ended = ended_subagents(&events);
    assert_eq!(ended.len(), 1, "{events:#?}");
    assert_eq!(&ended[0].0, "task1");
    assert!(ended[0].1, "a record that just stops is shown as finished");
    assert!(ended[0].2.as_deref().unwrap().contains("no end"));
    assert!(events.iter().any(|e| matches!(e, Event::ToolFinished { id, output } if id.as_str() == "sh1" && !output.is_error)), "the open shell call ends too");
    let mut conversation = crate::session::Conversation::new();
    events.iter().for_each(|e| conversation.apply(e));
    assert!(!conversation.working(), "the resumed session shows nothing running");
}

#[test]
fn history_of_an_aborted_turn_ends_open_work_as_interrupted() {
    let aborted = json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "terminal_reason": "aborted_tools"});
    let events = crate::claude_code::history(&subagent_transcript(&[aborted]));
    let ended = ended_subagents(&events);
    assert_eq!(ended.len(), 1, "{events:#?}");
    assert!(!ended[0].1, "an aborted turn leaves its subagent interrupted");
    assert!(ended[0].2.as_deref().unwrap().contains("Interrupted"));
}

#[test]
fn history_does_not_end_a_subagent_twice() {
    let done = json!({"type": "system", "subtype": "task_notification", "tool_use_id": "task1", "status": "completed", "summary": "counted"});
    let events = crate::claude_code::history(&subagent_transcript(&[done]));
    assert_eq!(ended_subagents(&events).len(), 1, "{events:#?}");
    assert_eq!(ended_subagents(&events)[0].2.as_deref(), Some("counted"));
}

/// The agent's own commands come with its init, so the composer can offer them after `/`.
#[test]
fn the_init_lists_the_agents_own_commands() {
    let mut mapper = Mapper::new();
    let events = mapper.line(
        r#"{"type":"system","subtype":"init","session_id":"s","slash_commands":["compact","review","goal"]}"#,
        Instant::now(),
    );
    let [Event::Started(started)] = events.as_slice() else { panic!("{events:?}") };
    assert_eq!(started.commands, ["compact", "review", "goal"]);
}

fn contexts(events: &[Event]) -> Vec<crate::session::ContextFill> {
    events.iter().filter_map(|e| if let Event::Context(context) = e { Some(*context) } else { None }).collect()
}

fn reply(model: &str, input: u64, cache: u64, parent: Option<&str>) -> String {
    json!({
        "type": "assistant",
        "parent_tool_use_id": parent,
        "message": {"id": format!("m-{input}"), "model": model, "content": [], "usage": {"input_tokens": input, "cache_read_input_tokens": cache, "output_tokens": 10}},
    })
    .to_string()
}

fn result_with_windows(windows: &[(&str, u64)]) -> String {
    let models: serde_json::Map<_, _> = windows.iter().map(|(name, window)| (name.to_string(), json!({"contextWindow": window}))).collect();
    json!({"type": "result", "subtype": "success", "result": "ok", "modelUsage": models}).to_string()
}

#[test]
fn a_recorded_turn_tells_how_full_the_context_is_and_then_its_window() {
    let told = contexts(&replay("plain"));
    let first = told.first().expect("the reply tells the context");
    assert!(first.used > 0 && first.window.is_none(), "the reply knows its tokens, not the window: {first:?}");
    assert_eq!(told.last().and_then(|c| c.window), Some(1_000_000), "the result tells the model's window");
}

#[test]
fn the_context_follows_the_latest_main_reply_and_the_window_its_model() {
    let mut mapper = Mapper::new();
    let now = Instant::now();
    let mut events = mapper.line(&reply("opus", 100, 1_000, None), now);
    events.extend(mapper.line(&reply("opus", 100, 1_000, None), now));
    events.extend(mapper.line(&reply("haiku", 5, 90_000, Some("toolu_sub")), now));
    events.extend(mapper.line(&reply("opus", 200, 2_000, None), now));
    events.extend(mapper.line(&result_with_windows(&[("haiku", 1_000_000), ("opus", 200_000)]), now));
    let told: Vec<_> = contexts(&events).iter().map(|c| (c.used, c.window)).collect();
    assert_eq!(told, [(1_110, None), (2_210, None), (2_210, Some(200_000))], "a repeat and a subagent's reply tell nothing");
}

#[test]
fn a_window_one_session_learned_shows_at_once_in_the_next() {
    let now = Instant::now();
    let mut first = Mapper::new();
    first.line(&reply("model-learned-once", 100, 0, None), now);
    first.line(&result_with_windows(&[("model-learned-once", 400_000)]), now);
    let told = contexts(&Mapper::new().line(&reply("model-learned-once", 300, 0, None), now));
    assert_eq!(told.iter().map(|c| (c.used, c.window)).collect::<Vec<_>>(), [(310, Some(400_000))]);
}

fn rate_limit(status: &str, kind: &str) -> String {
    json!({"type": "rate_limit_event", "rate_limit_info": {"status": status, "resetsAt": 1_790_000_000u64, "rateLimitType": kind}, "uuid": "u", "session_id": "s"}).to_string()
}

#[test]
fn the_usage_limit_is_told_when_it_changes() {
    use crate::session::{Limit, LimitState, LimitWindow};
    let mut mapper = Mapper::new();
    let now = Instant::now();
    let mut events = mapper.line(&rate_limit("allowed_warning", "seven_day_opus"), now);
    events.extend(mapper.line(&rate_limit("rejected", "five_hour"), now));
    events.extend(mapper.line(&rate_limit("rejected", "five_hour"), now));
    events.extend(mapper.line(&rate_limit("allowed", "five_hour"), now));
    events.extend(mapper.line(&rate_limit("something_new", "five_hour"), now));
    let told: Vec<Limit> = events.iter().filter_map(|e| if let Event::Limit(limit) = e { Some(*limit) } else { None }).collect();
    let at = Some(1_790_000_000);
    assert_eq!(told, [
        Limit { state: LimitState::Near, resets_at: at, window: Some(LimitWindow::Weekly) },
        Limit { state: LimitState::Reached, resets_at: at, window: Some(LimitWindow::FiveHour) },
        Limit { state: LimitState::Clear, resets_at: at, window: Some(LimitWindow::FiveHour) },
    ], "a repeat and an unknown status tell nothing");
    assert!(events.iter().all(|e| !matches!(e, Event::Warning(_))));
}
