//! The whole loop, over a scripted server: a session that streams text and thinking, runs tools, asks
//! permission, is interrupted, retries, and is resumed.
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{
    server::{self, FakeServer, Step, says, uses},
    support::{KEY, Rig, rig, text_of, turn_ends},
};
use crate::session::{
    Backend, ChoiceId, ChoiceKind, Command, Event, OpenRequest, PermissionMode, RequestId, ToolKind, ToolStatus, TurnOutcome,
};

fn tool_results(request: &Value) -> Vec<Value> {
    let messages = request["messages"].as_array().unwrap();
    messages
        .iter()
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter(|b| b["type"] == "tool_result")
        .collect()
}

fn finished<'a>(events: &'a [Event], id: &str) -> &'a crate::session::ToolOutput {
    events
        .iter()
        .find_map(|e| if let Event::ToolFinished { id: i, output } = e { (i.as_str() == id).then_some(output) } else { None })
        .unwrap_or_else(|| panic!("no ToolFinished for {id} in {events:#?}"))
}

#[test]
fn a_text_reply_streams_and_the_turn_completes() {
    let rig = rig(vec![Step::Sse(says("Hello there"))], PermissionMode::Ask);
    rig.send("Say hello");
    let events = rig.turns(1);
    assert!(matches!(&events[0], Event::Started(s) if s.model.as_deref() == Some("claude-opus-5-5") && s.mode == Some(PermissionMode::Ask)));
    assert_eq!(text_of(&events), "Hello there");
    let usage = events.iter().find_map(|e| if let Event::Usage(u) = e { Some(*u) } else { None }).expect("usage");
    assert_eq!((usage.input_tokens, usage.output_tokens, usage.cache_read_tokens, usage.cache_write_tokens), (100, 42, 5, 7));
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Completed);
    assert_eq!(turn_ends(&events)[0].summary.as_deref(), Some("Hello there"));
}

#[test]
fn the_request_carries_the_model_tools_thinking_and_cache_breakpoints_and_the_key_only_in_its_header() {
    let rig = rig(vec![Step::Sse(says("ok"))], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    let request = &rig.server.recorded()[0];
    assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/v1/messages"));
    assert_eq!(request.header("x-api-key"), Some(KEY));
    assert_eq!(request.header("anthropic-version"), Some("2023-06-01"));
    let body = &request.body;
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["thinking"]["type"], "adaptive");
    assert!(body.get("temperature").is_none() && body.get("budget_tokens").is_none(), "no sampling settings, no thinking budget");
    let names: Vec<_> = body["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["read", "list", "search", "edit", "write", "shell"]);
    assert_eq!(body["tools"].as_array().unwrap().last().unwrap()["cache_control"]["type"], "ephemeral", "the last tool is a cache breakpoint");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral", "so is the system prompt");
    let last = body["messages"].as_array().unwrap().last().unwrap()["content"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["cache_control"]["type"], "ephemeral", "and the end of the conversation");
    assert!(body["system"][0]["text"].as_str().unwrap().contains(rig.dir.path().file_name().unwrap().to_str().unwrap()));
    // The key is in the header and nowhere the app can see it.
    assert!(!format!("{events:?}").contains(KEY));
    assert!(!body.to_string().contains(KEY));
}

#[test]
fn thinking_streams_with_its_time_and_comes_back_signed_in_the_next_request() {
    let first = server::reply(vec![server::thinking_block(0, &["Let me ", "think."], "sig-abc"), server::text_block(1, &["Answer"])], "end_turn");
    let rig = rig(vec![Step::Sse(first), Step::Sse(says("Again"))], PermissionMode::Ask);
    rig.send("hard question");
    let events = rig.turns(1);
    let thinking: String = events.iter().filter_map(|e| if let Event::Thinking { delta, .. } = e { Some(delta.as_str()) } else { None }).collect();
    assert_eq!(thinking, "Let me think.");
    assert!(events.iter().any(|e| matches!(e, Event::ThinkingDone { .. })));
    let (think_block, text_block) = (
        events.iter().find_map(|e| if let Event::Thinking { block, .. } = e { Some(*block) } else { None }).unwrap(),
        events.iter().find_map(|e| if let Event::Text { block, .. } = e { Some(*block) } else { None }).unwrap(),
    );
    assert_ne!(think_block, text_block, "thinking and text are two blocks");
    rig.send("more");
    rig.turns(2);
    let second = &rig.server.recorded()[1].body;
    let assistant = &second["messages"][1];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(assistant["content"][0], json!({"type": "thinking", "thinking": "Let me think.", "signature": "sig-abc"}));
}

#[test]
fn one_tool_call_runs_and_its_result_goes_back_to_the_model() {
    let steps = vec![Step::Sse(uses(&[("toolu_1", "read", json!({"path": "note.txt"}))])), Step::Sse(says("It says hello."))];
    let rig = rig(steps, PermissionMode::Ask);
    std::fs::write(rig.dir.path().join("note.txt"), "hello\nworld\n").unwrap();
    rig.send("read note.txt");
    let events = rig.turns(1);
    let call = events.iter().find_map(|e| if let Event::ToolStarted(c) = e { Some(c.clone()) } else { None }).expect("ToolStarted");
    assert_eq!((call.name.as_str(), call.kind, call.status), ("read", ToolKind::Read, ToolStatus::Pending));
    assert!(events.iter().any(|e| matches!(e, Event::ToolInput { id, file, .. } if id.as_str() == "toolu_1" && file.as_deref() == Some("note.txt"))));
    assert!(events.iter().any(|e| matches!(e, Event::ToolTarget { file, .. } if file == "note.txt")));
    assert!(events.iter().any(|e| matches!(e, Event::ToolStatus { status: ToolStatus::Running, .. })));
    assert!(!events.iter().any(|e| matches!(e, Event::Permission(_))), "a read needs no question");
    let out = finished(&events, "toolu_1");
    assert!(!out.is_error);
    assert_eq!(out.text, "1\thello\n2\tworld\n");
    assert_eq!(text_of(&events), "It says hello.");
    let requests = rig.server.recorded();
    assert_eq!(requests.len(), 2);
    let results = tool_results(&requests[1].body);
    assert_eq!(results.len(), 1);
    assert_eq!((results[0]["tool_use_id"].as_str(), results[0]["content"].as_str()), (Some("toolu_1"), Some("1\thello\n2\tworld\n")));
    let assistant = &requests[1].body["messages"][1];
    assert_eq!(assistant["content"][0]["type"], "tool_use");
    assert_eq!(assistant["content"][0]["input"], json!({"path": "note.txt"}), "the input streamed in two pieces and joined");
}

#[test]
fn several_tool_calls_in_one_reply_all_run_and_come_back_in_one_message_in_order() {
    let calls = [("t1", "read", json!({"path": "a.txt"})), ("t2", "list", json!({})), ("t3", "search", json!({"pattern": "beta"}))];
    let rig = rig(vec![Step::Sse(uses(&calls)), Step::Sse(says("done"))], PermissionMode::Ask);
    std::fs::write(rig.dir.path().join("a.txt"), "alpha\nbeta\n").unwrap();
    rig.send("look around");
    let events = rig.turns(1);
    let starts: Vec<_> = events.iter().filter_map(|e| if let Event::ToolStarted(c) = e { Some(c.id.as_str().to_string()) } else { None }).collect();
    assert_eq!(starts, ["t1", "t2", "t3"]);
    assert!(finished(&events, "t2").text.contains("a.txt"));
    assert!(finished(&events, "t3").text.contains("a.txt:2: beta"));
    let requests = rig.server.recorded();
    let user_turns = requests[1].body["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "user").count();
    assert_eq!(user_turns, 2, "the question, and one message with every result");
    let ids: Vec<_> = tool_results(&requests[1].body).iter().map(|r| r["tool_use_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, ["t1", "t2", "t3"]);
}

#[test]
fn a_tool_that_fails_gives_the_model_an_error_it_can_read_and_the_turn_goes_on() {
    let steps = vec![Step::Sse(uses(&[("t1", "read", json!({"path": "missing.txt"}))])), Step::Sse(says("That file is not there."))];
    let rig = rig(steps, PermissionMode::Ask);
    rig.send("read missing.txt");
    let events = rig.turns(1);
    let out = finished(&events, "t1");
    assert!(out.is_error && out.text.contains("cannot read missing.txt"), "{out:?}");
    let results = tool_results(&rig.server.recorded()[1].body);
    assert_eq!(results[0]["is_error"], true);
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Completed);
}

#[test]
fn an_unknown_tool_and_an_input_that_is_not_json_are_errors_for_the_model_not_crashes() {
    let bad_json = server::reply(
        vec![
            vec![
                ("content_block_start".to_string(), json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t2", "name": "read", "input": {}}}).to_string()),
                ("content_block_delta".to_string(), json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"path\": "}}).to_string()),
                ("content_block_stop".to_string(), json!({"type": "content_block_stop", "index": 0}).to_string()),
            ],
        ],
        "tool_use",
    );
    let steps = vec![
        Step::Sse(uses(&[("t1", "teleport", json!({"where": "moon"}))])),
        Step::Sse(bad_json),
        Step::Sse(says("sorry")),
    ];
    let rig = rig(steps, PermissionMode::Ask);
    rig.send("go");
    let events = rig.turns(1);
    let unknown = finished(&events, "t1");
    assert!(unknown.is_error && unknown.text.contains("no tool named teleport") && unknown.text.contains("read, list"), "{unknown:?}");
    let malformed = finished(&events, "t2");
    assert!(malformed.is_error && malformed.text.contains("not valid JSON"), "{malformed:?}");
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Completed);
}

fn permission_request(events: &[Event]) -> Option<crate::session::PermissionRequest> {
    events.iter().find_map(|e| if let Event::Permission(p) = e { Some(p.clone()) } else { None })
}

fn write_steps() -> Vec<Step> {
    vec![
        Step::Sse(uses(&[("w1", "write", json!({"path": "out/new.txt", "content": "made"}))])),
        Step::Sse(says("finished")),
    ]
}

#[test]
fn a_change_asks_first_and_allow_lets_it_run() {
    let rig = rig(write_steps(), PermissionMode::Ask);
    rig.send("write it");
    let events = rig.wait_for("a permission request", |e| permission_request(e).is_some());
    let request = permission_request(&events).unwrap();
    assert_eq!((request.call.name.as_str(), request.call.kind), ("write", ToolKind::Write));
    assert_eq!(request.choices.iter().map(|c| c.kind).collect::<Vec<_>>(), [ChoiceKind::Allow, ChoiceKind::AllowAlways, ChoiceKind::Deny]);
    assert!(!rig.dir.path().join("out/new.txt").exists(), "nothing ran before the answer");
    rig.command(Command::Answer { request: request.id.clone(), choice: request.choices[0].id.clone() });
    let events = rig.turns(1);
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("out/new.txt")).unwrap(), "made", "the folder was made too");
    assert!(!finished(&events, "w1").is_error);
}

#[test]
fn a_denied_change_does_not_run_and_the_model_is_told_so() {
    let rig = rig(write_steps(), PermissionMode::Ask);
    rig.send("write it");
    let events = rig.wait_for("a permission request", |e| permission_request(e).is_some());
    let request = permission_request(&events).unwrap();
    rig.command(Command::Answer { request: request.id, choice: ChoiceId::new("deny") });
    let events = rig.turns(1);
    assert!(!rig.dir.path().join("out/new.txt").exists());
    let out = finished(&events, "w1");
    assert!(out.is_error && out.text.contains("did not allow"), "{out:?}");
    let results = tool_results(&rig.server.recorded()[1].body);
    assert_eq!(results[0]["is_error"], true);
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Completed, "the model gets to answer the denial");
}

#[test]
fn always_allow_lets_the_same_tool_run_again_without_a_question() {
    let steps = vec![
        Step::Sse(uses(&[("w1", "write", json!({"path": "a.txt", "content": "1"}))])),
        Step::Sse(uses(&[("w2", "write", json!({"path": "b.txt", "content": "2"}))])),
        Step::Sse(says("both")),
    ];
    let rig = rig(steps, PermissionMode::Ask);
    rig.send("write two files");
    let events = rig.wait_for("a permission request", |e| permission_request(e).is_some());
    let request = permission_request(&events).unwrap();
    rig.command(Command::Answer { request: request.id, choice: ChoiceId::new("allow-always") });
    let events = rig.turns(1);
    assert_eq!(events.iter().filter(|e| matches!(e, Event::Permission(_))).count(), 1, "asked once");
    assert!(rig.dir.path().join("b.txt").exists());
}

#[test]
fn plan_mode_refuses_changes_without_asking_and_bypass_and_accept_edits_do_not_ask() {
    for (mode, runs, asks) in [(PermissionMode::Plan, false, false), (PermissionMode::Bypass, true, false), (PermissionMode::AcceptEdits, true, false)] {
        let rig = rig(write_steps(), mode);
        rig.send("write it");
        let events = rig.turns(1);
        assert_eq!(rig.dir.path().join("out/new.txt").exists(), runs, "{mode:?}");
        assert_eq!(permission_request(&events).is_some(), asks, "{mode:?}");
        if !runs {
            assert!(finished(&events, "w1").text.contains("Plan mode"));
        }
    }
}

#[test]
fn a_shell_command_needs_an_answer_even_when_edits_are_accepted() {
    let steps = vec![Step::Sse(uses(&[("s1", "shell", json!({"command": "echo hi > ran.txt"}))])), Step::Sse(says("ok"))];
    let rig = rig(steps, PermissionMode::AcceptEdits);
    rig.send("run it");
    let events = rig.wait_for("a permission request", |e| permission_request(e).is_some());
    assert_eq!(permission_request(&events).unwrap().call.kind, ToolKind::Shell);
    rig.command(Command::Answer { request: permission_request(&events).unwrap().id, choice: ChoiceId::new("allow") });
    rig.turns(1);
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("ran.txt")).unwrap().trim(), "hi");
}

#[test]
fn a_mode_switched_during_the_session_applies_to_the_next_call() {
    let rig = rig(write_steps(), PermissionMode::Ask);
    rig.command(Command::SetPermissionMode { mode: PermissionMode::Bypass });
    rig.send("write it");
    let events = rig.turns(1);
    assert!(permission_request(&events).is_none());
    assert!(rig.dir.path().join("out/new.txt").exists());
}

#[test]
fn interrupting_a_stream_stops_it_keeps_what_streamed_and_frees_the_session_for_the_next_message() {
    let partial = vec![server::start(10), server::text_block(0, &["Half of an ans"])[0].clone(), server::text_block(0, &["Half of an ans"])[1].clone()];
    let rig = rig(vec![Step::Hang(partial), Step::Sse(says("Fresh start"))], PermissionMode::Ask);
    rig.send("tell me a lot");
    rig.wait_for("the first words", |e| text_of(e).contains("Half of an ans"));
    let started = Instant::now();
    rig.command(Command::Interrupt);
    let events = rig.turns(1);
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Interrupted);
    assert!(started.elapsed() < Duration::from_secs(2), "the interrupt took {:?}", started.elapsed());
    let limit = Instant::now() + Duration::from_secs(5);
    while !rig.server.hung_up.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(rig.server.hung_up.load(std::sync::atomic::Ordering::SeqCst), "the connection was closed, not left open");
    rig.send("try again");
    let events = rig.turns(2);
    assert_eq!(turn_ends(&events)[1].outcome, TurnOutcome::Completed);
    let second = &rig.server.recorded()[1].body["messages"];
    assert_eq!(second[1]["role"], "assistant");
    assert_eq!(second[1]["content"][0]["text"], "Half of an ans", "what streamed before the interrupt stays in the conversation");
}

#[test]
fn interrupting_while_a_tool_waits_for_an_answer_cancels_the_question() {
    let rig = rig(write_steps(), PermissionMode::Ask);
    rig.send("write it");
    let events = rig.wait_for("a permission request", |e| permission_request(e).is_some());
    let request = permission_request(&events).unwrap();
    rig.command(Command::Interrupt);
    let events = rig.turns(1);
    assert!(events.iter().any(|e| matches!(e, Event::PermissionCancelled(id) if *id == request.id)));
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Interrupted);
    assert!(!rig.dir.path().join("out/new.txt").exists());
}

#[test]
fn interrupting_a_running_command_kills_it() {
    let steps = vec![Step::Sse(uses(&[("s1", "shell", json!({"command": "sleep 30"}))]))];
    let rig = rig(steps, PermissionMode::Bypass);
    rig.send("wait");
    rig.wait_for("the command to run", |e| e.iter().any(|e| matches!(e, Event::ToolStatus { status: ToolStatus::Running, .. })));
    let started = Instant::now();
    rig.command(Command::Interrupt);
    let events = rig.turns(1);
    assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Interrupted);
    assert!(finished(&events, "s1").is_error);
}

#[test]
fn a_stream_that_is_not_a_stream_fails_the_turn_with_a_reason() {
    for (bytes, why) in [
        (b"event: message_start\ndata: {not json\n\n".to_vec(), "did not read"),
        (b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{}}}\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\n".to_vec(), "ended before the reply was complete"),
        (b"<html>an error page</html>".to_vec(), "ended before the reply was complete"),
    ] {
        let rig = rig(vec![Step::Raw { bytes, chunk: 9 }], PermissionMode::Ask);
        rig.send("hi");
        let events = rig.turns(1);
        let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
        assert!(reason.contains(why), "{reason}");
    }
}

#[test]
fn a_rate_limit_waits_and_tries_again() {
    let limited = Step::Status { code: 429, headers: vec![("retry-after", "0".into())], body: server::error_body("rate_limit_error", "slow down") };
    let rig = rig(vec![limited, Step::Sse(says("through"))], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    assert!(events.iter().any(|e| matches!(e, Event::Warning(w) if w.contains("rate limiting") && w.contains("Trying again"))), "{events:#?}");
    assert_eq!(text_of(&events), "through");
    assert_eq!(rig.server.recorded().len(), 2);
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Completed);
}

#[test]
fn a_server_that_keeps_failing_ends_the_turn_after_the_retries() {
    let down = || Step::Status { code: 529, headers: vec![], body: server::error_body("overloaded_error", "Overloaded") };
    let rig = rig(vec![down(), down(), down(), down()], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
    assert!(reason.contains("Overloaded") && reason.contains("529"), "{reason}");
    assert_eq!(rig.server.recorded().len(), 3, "the first try and two retries");
}

#[test]
fn a_refused_key_or_a_bad_request_is_not_retried_and_never_shows_the_key() {
    for (code, kind, message) in [(401, "authentication_error", "invalid x-api-key"), (400, "invalid_request_error", "max_tokens: too large"), (404, "not_found_error", "model: nope")] {
        let rig = rig(vec![Step::Status { code, headers: vec![], body: server::error_body(kind, message) }], PermissionMode::Ask);
        rig.send("hi");
        let events = rig.turns(1);
        let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
        assert!(reason.contains(message), "{reason}");
        assert_eq!(rig.server.recorded().len(), 1, "{code} is not retried");
        assert!(!format!("{events:?}").contains(KEY));
    }
}

#[test]
fn a_reply_cut_off_by_the_token_limit_fails_the_turn_and_leaves_no_dangling_tool_call() {
    let cut = server::reply(vec![server::text_block(0, &["Let me"]), server::tool_block(1, "t1", "write", &json!({"path": "x", "content": "y"}))], "max_tokens");
    let rig = rig(vec![Step::Sse(cut), Step::Sse(says("fine"))], PermissionMode::Bypass);
    rig.send("big job");
    let events = rig.turns(1);
    let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
    assert!(reason.contains("token limit"), "{reason}");
    assert!(!rig.dir.path().join("x").exists(), "the cut-off call did not run");
    rig.send("go on");
    rig.turns(2);
    let second = rig.server.recorded()[1].body.to_string();
    assert!(!second.contains("tool_use"), "the next request has no tool call without a result: {second}");
}

#[test]
fn a_refusal_ends_the_turn_as_a_failure() {
    let refusal = server::reply(vec![], "refusal");
    let rig = rig(vec![Step::Sse(refusal)], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
    assert!(reason.contains("declined"), "{reason}");
}

#[test]
fn a_loop_that_never_stops_calling_tools_is_stopped() {
    let mut options = super::support::options();
    options.max_steps = 3;
    let steps = (0..5).map(|i| Step::Sse(uses(&[(&format!("t{i}"), "list", json!({}))]))).collect();
    let mut rig = rig(steps, PermissionMode::Ask);
    let model = crate::own::Anthropic::new(crate::own::Secret::new(KEY)).with_base(rig.server.url.clone());
    rig.agent = crate::own::OwnAgent::new(std::sync::Arc::new(model), options);
    rig.session = None;
    rig.events.lock().unwrap().clear();
    rig.open(OpenRequest::default());
    rig.send("loop");
    let events = rig.turns(1);
    let TurnOutcome::Failed(reason) = &turn_ends(&events)[0].outcome else { panic!("{events:#?}") };
    assert!(reason.contains("3 steps"), "{reason}");
}

#[test]
fn switching_the_model_applies_to_the_next_request() {
    let rig = rig(vec![Step::Sse(says("a")), Step::Sse(says("b"))], PermissionMode::Ask);
    rig.send("one");
    rig.turns(1);
    rig.command(Command::SetModel { model: "claude-haiku-4-5".into() });
    rig.send("two");
    rig.turns(2);
    let requests = rig.server.recorded();
    assert_eq!((requests[0].body["model"].as_str(), requests[1].body["model"].as_str()), (Some("claude-opus-5-5"), Some("claude-haiku-4-5")));
    assert_eq!(requests[1].body["thinking"]["type"], "enabled", "Haiku 4.5 takes a thinking budget, not adaptive thinking");
    assert!(requests[1].body["thinking"]["budget_tokens"].as_u64().unwrap() < requests[1].body["max_tokens"].as_u64().unwrap());
}

#[test]
fn a_message_sent_while_a_turn_runs_waits_and_runs_next() {
    let partial = vec![server::start(10), server::text_block(0, &["working"])[0].clone(), server::text_block(0, &["working"])[1].clone()];
    let rig = rig(vec![Step::Hang(partial), Step::Sse(says("second answer"))], PermissionMode::Ask);
    rig.send("first");
    rig.wait_for("the first words", |e| text_of(e).contains("working"));
    rig.send("second");
    rig.command(Command::Interrupt);
    let events = rig.turns(2);
    assert_eq!(turn_ends(&events)[0].outcome, TurnOutcome::Interrupted);
    assert_eq!(turn_ends(&events)[1].outcome, TurnOutcome::Completed);
    assert!(text_of(&events).ends_with("second answer"));
}

#[test]
fn a_session_is_saved_listed_resumed_and_shown_as_history() {
    let mut rig = rig(vec![Step::Sse(says("First answer")), Step::Sse(says("Second answer"))], PermissionMode::Ask);
    rig.send("What is in this project?");
    rig.turns(1);
    let id = rig.session_id();
    let listed = rig.agent.sessions(rig.project.as_ref()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].id.clone(), listed[0].title.as_str()), (id.clone(), "What is in this project?"));
    // Close it and open it again from the record.
    rig.session = None;
    rig.wait_for("the session to end", |e| e.iter().any(|e| matches!(e, Event::Ended(_))));
    rig.events.lock().unwrap().clear();
    rig.open(OpenRequest { resume: Some(id.clone()), ..OpenRequest::default() });
    rig.send("And now?");
    rig.turns(1);
    assert!(matches!(&rig.events()[0], Event::Started(s) if s.session == id), "the same session id");
    let messages = &rig.server.recorded()[1].body["messages"];
    assert_eq!(messages[0]["content"][0]["text"], "What is in this project?");
    assert_eq!(messages[1]["content"][0]["text"], "First answer");
    assert_eq!(messages[2]["content"][0]["text"], "And now?");
    let history = rig.agent.history(rig.project.as_ref(), &id).unwrap();
    assert!(matches!(&history[0], Event::UserMessage { text } if text == "What is in this project?"));
    assert_eq!(text_of(&history), "First answerSecond answer");
    assert_eq!(rig.agent.sessions(rig.project.as_ref()).unwrap().len(), 1, "one session, not two");
}

#[test]
fn resuming_a_session_that_does_not_exist_is_an_error() {
    let rig = rig(vec![], PermissionMode::Ask);
    for id in ["nope", "../../etc/passwd", ""] {
        let sink: crate::session::EventSink = std::sync::Arc::new(|_| {});
        let result = rig.agent.open(rig.project.clone(), OpenRequest { resume: Some(crate::session::SessionId::new(id)), ..OpenRequest::default() }, sink);
        assert!(matches!(result, Err(crate::session::SessionError::Read(_))), "{id}");
    }
}

#[test]
fn dropping_the_session_ends_it_and_stops_the_loop() {
    let mut rig = rig(vec![], PermissionMode::Ask);
    rig.wait_for("started", |e| !e.is_empty());
    rig.session = None;
    let events = rig.wait_for("the end", |e| e.iter().any(|e| matches!(e, Event::Ended(_))));
    assert!(matches!(events.last(), Some(Event::Ended(crate::session::EndReason::Closed))));
}

#[test]
fn an_agent_with_no_key_says_what_to_set_when_a_session_opens() {
    let _guard = crate::testing::spawn_lock();
    let saved: Vec<_> = ["ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"].iter().map(|k| (*k, std::env::var(k).ok())).collect();
    for (k, _) in &saved {
        // SAFETY: the lock above keeps the other tests that spawn from running; no test reads these.
        unsafe { std::env::remove_var(k) };
    }
    let agent = crate::own::OwnAgent::from_env();
    for (k, v) in saved {
        if let Some(v) = v {
            unsafe { std::env::set_var(k, v) };
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let project: std::sync::Arc<dyn lathe_project::Project> = std::sync::Arc::new(lathe_project::LocalProject::open(dir.path()).unwrap());
    let error = agent.open(project, OpenRequest::default(), std::sync::Arc::new(|_| {})).err().expect("no key");
    assert!(error.to_string().contains("ANTHROPIC_API_KEY"), "{error}");
}

#[test]
fn the_backend_says_what_it_can_do() {
    let rig = rig(vec![], PermissionMode::Ask);
    let caps = rig.agent.capabilities();
    assert!(caps.resume && caps.interrupt && caps.thinking);
    assert!(!caps.subagents && !caps.todos, "no subagents yet");
    assert_eq!(caps.models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"]);
    assert!(caps.permission_modes.contains(&PermissionMode::Plan) && caps.permission_modes.contains(&PermissionMode::Bypass));
    assert_eq!(rig.agent.name(), "lathe");
    let _ = (&rig as &Rig, RequestId::new("x"), FakeServer::start(vec![]));
}
