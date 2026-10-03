//! The conversation with an agent: handshake, sign-in, turns, questions, modes and models, and its end.
use serde_json::json;

use super::{Run, chunk_in, fail, respond, update, update_in};
use crate::{
    acp::protocol::{Found, Goal},
    session::{
        Attachment, ChoiceId, ChoiceKind, Command, EndReason, Event, OpenRequest, PermissionMode, RequestId, SessionError,
        SessionId, Started, ToolStatus, TurnOutcome,
    },
};

fn started(model: Option<&str>, mode: Option<PermissionMode>, commands: &[&str]) -> Event {
    Event::Started(Started {
        session: SessionId::new("s1"),
        model: model.map(str::to_string),
        mode,
        commands: commands.iter().map(|c| c.to_string()).collect(),
    })
}

#[test]
fn the_handshake_initializes_then_starts_a_session_in_the_projects_folder() {
    let mut run = Run::open();
    let init = run.last().clone();
    assert_eq!((init["id"].clone(), init["method"].clone()), (json!(0), json!("initialize")));
    assert_eq!(init["params"]["protocolVersion"], 1);
    assert_eq!(init["params"]["clientCapabilities"]["terminal"], false, "atelier serves no terminal");
    assert_eq!(init["params"]["clientCapabilities"]["fs"]["readTextFile"], false, "nor files");
    assert_eq!(init["params"]["clientInfo"]["name"], "atelier");

    run.initialized(json!({}));
    assert_eq!(run.last()["method"], "session/new");
    assert_eq!(run.last()["params"], json!({ "cwd": "/work/project", "mcpServers": [] }));
    assert!(run.events().is_empty(), "nothing has started yet");
}

#[test]
fn a_started_session_says_its_id_model_and_mode() {
    let mut run = Run::ready();
    assert_eq!(run.events(), vec![started(Some("auto"), Some(PermissionMode::Ask), &[])]);
}

#[test]
fn a_session_with_no_model_option_takes_its_model_from_acps_model_list() {
    let mut run = Run::open();
    run.initialized(json!({}));
    run.agent(respond(1, json!({ "sessionId": "s1", "models": { "currentModelId": "fast", "availableModels": [] } })));
    assert_eq!(run.events(), vec![started(Some("fast"), None, &[])]);
    run.command(Command::SetModel { model: "auto".into() });
    assert_eq!(run.last()["method"], "session/set_model", "no config option: the unstable model call");
    assert_eq!(run.last()["params"], json!({ "sessionId": "s1", "modelId": "auto" }));
}

#[test]
fn an_agent_that_needs_a_sign_in_is_authenticated_once_and_asked_again() {
    let mut run = Run::open();
    run.initialized(json!({}));
    run.agent(fail(1, -32000, "Authentication required"));
    assert_eq!(run.last()["method"], "authenticate");
    assert_eq!(run.last()["params"], json!({ "methodId": "cursor_login" }));
    run.agent(respond(2, json!({})));
    assert_eq!(run.last()["method"], "session/new", "asked again once signed in");
    assert_eq!(run.sent("session/new").len(), 2);
}

#[test]
fn a_terminal_method_is_never_authenticated_the_agent_one_is() {
    let mut run = Run::open();
    run.agent(respond(0, json!({
        "protocolVersion": 1,
        "authMethods": [
            { "id": "terminal-login", "name": "Log in from the terminal", "type": "terminal", "args": ["--login"] },
            { "id": "agent-login", "name": "Agent login", "type": "agent" },
        ],
    })));
    run.agent(fail(1, -32000, "Authentication required"));
    assert_eq!(run.last()["method"], "authenticate");
    assert_eq!(run.last()["params"], json!({ "methodId": "agent-login" }), "a terminal method is for the client's own terminal, not for `authenticate`");
}

#[test]
fn an_agent_that_offers_only_a_terminal_method_is_signed_out_and_not_authenticated() {
    let mut run = Run::open();
    run.command(Command::send("hello"));
    run.agent(respond(0, json!({
        "protocolVersion": 1,
        "authMethods": [{ "id": "terminal-login", "name": "Log in from the terminal", "type": "terminal" }],
    })));
    run.agent(fail(1, -32000, "Authentication required"));
    assert!(run.sent("authenticate").is_empty(), "the protocol forbids it");
    let events = run.events();
    assert_eq!(events[0], Event::SignedOut, "the reader signs in with the agent's own login, and the notice says so");
    assert!(run.done);
}

#[test]
fn an_agent_still_signed_out_after_the_sign_in_ends_the_session_with_its_own_words() {
    let mut run = Run::open();
    run.command(Command::send("hello"));
    run.initialized(json!({}));
    run.agent(fail(1, -32000, "Authentication required"));
    run.agent(respond(2, json!({})));
    run.agent(json!({ "jsonrpc": "2.0", "id": 3, "error": { "code": -32000, "message": "Authentication required", "data": { "message": "Run 'agent login' first." } } }));
    assert_eq!(run.sent("authenticate").len(), 1, "atelier signs in once");
    let events = run.events();
    assert_eq!(events[0], Event::SignedOut, "the sign-in comes first, so the failure that follows adds nothing");
    assert!(matches!(&events[1], Event::TurnEnded(end) if end.outcome == TurnOutcome::Failed("Run 'agent login' first.".into())), "the message sent fails");
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Failed("Run 'agent login' first.".into()))));
    assert!(run.done, "the agent is stopped");
    assert!(matches!(run.protocol.command(Command::send("again"), std::time::Instant::now()), Err(SessionError::Closed)));
}

#[test]
fn a_sign_in_that_runs_out_in_the_middle_of_a_session_fails_the_turn_as_signed_out() {
    let mut run = Run::ready();
    run.events();
    run.command(Command::send("hello"));
    let id = run.sent("session/prompt")[0]["id"].as_u64().expect("the prompt has an id");
    run.agent(fail(id, -32000, "Authentication required"));
    let events = run.events();
    assert_eq!(events[0], Event::SignedOut);
    assert!(matches!(&events[1], Event::TurnEnded(end) if matches!(end.outcome, TurnOutcome::Failed(_))), "{events:?}");
    assert!(!run.done, "the agent still runs: signing in and asking again is all it needs");
}

#[test]
fn another_error_of_a_turn_is_not_a_missing_sign_in() {
    let mut run = Run::ready();
    run.events();
    run.command(Command::send("hello"));
    let id = run.sent("session/prompt")[0]["id"].as_u64().expect("the prompt has an id");
    run.agent(fail(id, -32603, "Internal error"));
    assert!(!run.events().contains(&Event::SignedOut));
}

#[test]
fn messages_sent_before_the_session_is_ready_are_prompted_one_turn_at_a_time() {
    let mut run = Run::open();
    run.command(Command::send("first"));
    run.command(Command::Send {
        text: "second".into(),
        attachments: vec![Attachment::File { path: "src/a.rs".into() }],
    });
    assert!(run.sent("session/prompt").is_empty(), "no session yet");
    run.initialized(json!({}));
    run.agent(respond(1, json!({ "sessionId": "s1" })));
    let prompts = run.sent("session/prompt");
    assert_eq!(prompts.len(), 1, "one turn at a time");
    assert_eq!(prompts[0]["params"], json!({ "sessionId": "s1", "prompt": [{ "type": "text", "text": "first" }] }));

    run.agent(respond(2, json!({ "stopReason": "end_turn" })));
    let prompts = run.sent("session/prompt");
    assert_eq!(prompts.len(), 2, "the next message goes when the turn ends");
    assert_eq!(prompts[1]["params"]["prompt"][0]["text"], "second\n\nFile: src/a.rs");
}

#[test]
fn a_message_sent_while_a_turn_runs_waits_for_it() {
    let mut run = Run::ready();
    run.command(Command::send("one")).command(Command::send("two"));
    assert_eq!(run.sent("session/prompt").len(), 1);
    run.agent(respond(2, json!({ "stopReason": "end_turn" })));
    assert_eq!(run.sent("session/prompt").len(), 2);
}

#[test]
fn each_stop_reason_ends_the_turn_in_ateliers_words() {
    for (reason, want) in [
        ("end_turn", TurnOutcome::Completed),
        ("cancelled", TurnOutcome::Interrupted),
        ("refusal", TurnOutcome::Failed("The model refused to go on.".into())),
        ("max_tokens", TurnOutcome::Failed("The reply reached the model's token limit.".into())),
        ("max_turn_requests", TurnOutcome::Failed("The turn reached the agent's limit of model requests.".into())),
    ] {
        let mut run = Run::ready();
        run.command(Command::send("go"));
        run.events();
        run.agent(respond(2, json!({ "stopReason": reason })));
        assert!(matches!(run.events().last(), Some(Event::TurnEnded(end)) if end.outcome == want), "{reason}");
    }
}

#[test]
fn a_prompt_the_agent_refuses_fails_its_turn_and_the_session_goes_on() {
    let mut run = Run::ready();
    run.command(Command::send("go"));
    run.agent(fail(2, -32603, "model unavailable"));
    assert!(matches!(run.events().last(), Some(Event::TurnEnded(end)) if end.outcome == TurnOutcome::Failed("model unavailable".into())));
    run.command(Command::send("again"));
    assert_eq!(run.sent("session/prompt").len(), 2);
}

fn ask_permission(run: &mut Run, id: serde_json::Value) {
    run.agent(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/request_permission",
        "params": {
            "sessionId": "s1",
            "toolCall": { "toolCallId": "t1", "title": "rm -rf build", "kind": "execute", "rawInput": { "command": "rm -rf build" } },
            "options": [
                { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
                { "optionId": "allow-always", "name": "Always allow", "kind": "allow_always" },
                { "optionId": "reject-once", "name": "Reject", "kind": "reject_once" },
            ],
        },
    }));
}

#[test]
fn a_permission_question_carries_its_call_and_the_agents_choices_and_takes_one_answer() {
    let mut run = Run::ready();
    run.command(Command::send("clean up"));
    run.events();
    ask_permission(&mut run, json!("p-7"));
    let events = run.events();
    assert!(matches!(&events[0], Event::ToolStarted(call) if call.id.as_str() == "t1"), "an unnamed call is announced first");
    let Event::Permission(request) = &events[1] else { panic!("a question") };
    assert_eq!(request.id, RequestId::new("p-7"));
    assert_eq!(request.call.input, json!({ "command": "rm -rf build" }));
    let kinds: Vec<_> = request.choices.iter().map(|c| (c.id.as_str(), c.kind)).collect();
    assert_eq!(kinds, [("allow-once", ChoiceKind::Allow), ("allow-always", ChoiceKind::AllowAlways), ("reject-once", ChoiceKind::Deny)]);

    run.command(Command::Answer { request: RequestId::new("p-7"), choice: ChoiceId::new("allow-always") });
    assert_eq!(run.last(), &json!({ "jsonrpc": "2.0", "id": "p-7", "result": { "outcome": { "outcome": "selected", "optionId": "allow-always" } } }));
    let again = run.protocol.command(Command::Answer { request: RequestId::new("p-7"), choice: ChoiceId::new("allow-once") }, std::time::Instant::now());
    assert!(matches!(again, Err(SessionError::Unsupported(_))), "answered once");
}

#[test]
fn a_questions_content_is_its_reason_and_not_the_calls_output() {
    let mut run = Run::ready();
    run.command(Command::send("clean up"));
    run.agent(update(json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "rm", "kind": "execute", "status": "in_progress" })));
    run.events();
    run.agent(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "session/request_permission",
        "params": {
            "sessionId": "s1",
            "toolCall": { "toolCallId": "t1", "status": "pending", "content": [{ "type": "content", "content": { "type": "text", "text": "Not in allowlist: rm" } }] },
            "options": [{ "optionId": "reject-once", "name": "Reject", "kind": "reject_once" }],
        },
    }));
    let events = run.events();
    let [Event::Permission(request)] = events.as_slice() else { panic!("only the question: {events:?}") };
    assert_eq!(request.reason.as_deref(), Some("Not in allowlist: rm"));
    assert_eq!(request.call.status, ToolStatus::Running, "the question does not take the call back to pending");
    run.command(Command::Answer { request: RequestId::new("4"), choice: ChoiceId::new("reject-once") });
    run.agent(update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed" })));
    assert!(matches!(run.events().as_slice(), [Event::ToolFinished { output, .. }] if output.text.is_empty()));
}

#[test]
fn a_model_with_options_in_its_id_goes_by_its_name_and_is_set_by_its_whole_id() {
    let mut run = Run::open();
    run.initialized(json!({}));
    run.agent(respond(1, json!({
        "sessionId": "s1",
        "models": {
            "currentModelId": "default[]",
            "availableModels": [{ "modelId": "default[]" }, { "modelId": "composer-2.5[fast=true]" }],
        },
    })));
    assert_eq!(run.events(), vec![started(Some("default"), None, &[])]);
    run.command(Command::SetModel { model: "composer-2.5".into() });
    assert_eq!(run.last()["params"], json!({ "sessionId": "s1", "modelId": "composer-2.5[fast=true]" }));
    run.agent(respond(2, json!(null)));
    assert_eq!(run.events(), vec![started(Some("composer-2.5"), None, &[])]);
}

#[test]
fn a_numbered_question_is_answered_with_its_number() {
    let mut run = Run::ready();
    run.command(Command::send("go"));
    ask_permission(&mut run, json!(41));
    run.command(Command::Answer { request: RequestId::new("41"), choice: ChoiceId::new("reject-once") });
    assert_eq!(run.last()["id"], json!(41));
}

#[test]
fn a_question_with_no_turn_running_is_declined() {
    let mut run = Run::ready();
    run.events();
    ask_permission(&mut run, json!("p-1"));
    assert_eq!(run.last(), &json!({ "jsonrpc": "2.0", "id": "p-1", "result": { "outcome": { "outcome": "cancelled" } } }));
    assert!(run.events().is_empty());
}

#[test]
fn an_interrupt_cancels_the_turn_and_withdraws_the_waiting_question() {
    let mut run = Run::ready();
    run.command(Command::send("go"));
    ask_permission(&mut run, json!("p-1"));
    run.events();
    run.command(Command::Interrupt);
    let cancel = run.sent("session/cancel");
    assert_eq!(cancel.len(), 1);
    assert!(cancel[0].get("id").is_none(), "a notification");
    assert_eq!(cancel[0]["params"], json!({ "sessionId": "s1" }));
    assert_eq!(run.last()["result"]["outcome"]["outcome"], "cancelled", "the question is answered as cancelled");
    assert_eq!(run.events(), vec![Event::PermissionCancelled(RequestId::new("p-1"))]);

    run.agent(respond(2, json!({ "stopReason": "cancelled" })));
    let events = run.events();
    assert!(matches!(events.iter().find(|e| matches!(e, Event::ToolFinished { .. })), Some(Event::ToolFinished { output, .. }) if output.is_error), "the open call fails");
    assert!(matches!(events.last(), Some(Event::TurnEnded(end)) if end.outcome == TurnOutcome::Interrupted));
}

#[test]
fn an_interrupt_before_the_session_is_ready_drops_the_waiting_message() {
    let mut run = Run::open();
    run.command(Command::send("go")).command(Command::Interrupt);
    assert!(matches!(run.events().as_slice(), [Event::TurnEnded(end)] if end.outcome == TurnOutcome::Interrupted));
    run.initialized(json!({}));
    run.agent(respond(1, json!({ "sessionId": "s1" })));
    assert!(run.sent("session/prompt").is_empty());
}

#[test]
fn atelier_serves_no_method_of_its_own_and_says_so() {
    let mut run = Run::ready();
    run.agent(json!({ "jsonrpc": "2.0", "id": 9, "method": "fs/read_text_file", "params": { "path": "/etc/hosts" } }));
    assert_eq!(run.last()["id"], 9);
    assert_eq!(run.last()["error"]["code"], -32601);
}

#[test]
fn a_resumed_session_loads_it_and_does_not_repeat_its_history() {
    let mut run = Run::new(Goal::Open(OpenRequest { resume: Some(SessionId::new("old")), ..OpenRequest::default() }));
    run.initialized(json!({ "loadSession": true }));
    assert_eq!(run.last()["method"], "session/load");
    assert_eq!(run.last()["params"], json!({ "sessionId": "old", "cwd": "/work/project", "mcpServers": [] }));
    run.agent(chunk_in("old", "user_message_chunk", "earlier")).agent(chunk_in("old", "agent_message_chunk", "reply"));
    run.agent(respond(1, json!(null)));
    let events = run.events();
    assert_eq!(events.len(), 1, "only the start: the app read the history itself");
    assert!(matches!(&events[0], Event::Started(s) if s.session.as_str() == "old"));
}

#[test]
fn an_agent_that_does_not_load_sessions_does_not_offer_to_resume() {
    use crate::session::Backend as _;
    let loads = crate::acp::Acp::new(super::agent());
    let does_not = crate::acp::Acp::new(crate::acp::AcpAgent { resume: false, ..super::agent() });
    assert!(loads.capabilities().resume);
    assert!(!does_not.capabilities().resume);
}

#[test]
fn an_agent_that_cannot_load_cannot_resume() {
    let mut run = Run::new(Goal::Open(OpenRequest { resume: Some(SessionId::new("old")), ..OpenRequest::default() }));
    run.initialized(json!({}));
    assert_eq!(run.events(), vec![Event::Ended(EndReason::Failed("this agent cannot resume a session".into()))]);
}

#[test]
fn the_model_and_mode_a_session_opens_with_are_set_once_it_is_ready() {
    let request = OpenRequest { model: Some("fast".into()), mode: Some(PermissionMode::Plan), ..OpenRequest::default() };
    let mut run = Run::new(Goal::Open(request));
    run.initialized(json!({}));
    run.agent(respond(1, json!({
        "sessionId": "s1",
        "modes": { "currentModeId": "agent", "availableModes": [] },
        "configOptions": [{ "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "auto", "options": [] }],
    })));
    assert_eq!(run.sent("session/set_config_option")[0]["params"], json!({ "sessionId": "s1", "configId": "model", "value": "fast" }));
    assert_eq!(run.sent("session/set_mode")[0]["params"], json!({ "sessionId": "s1", "modeId": "plan" }));
    run.events();
    run.agent(respond(2, json!({}))).agent(respond(3, json!(null)));
    assert_eq!(run.events(), vec![started(Some("fast"), Some(PermissionMode::Ask), &[]), started(Some("fast"), Some(PermissionMode::Plan), &[])]);
}

/// The first turn runs on the model and mode the session opened with: its prompt waits for the agent's answers.
#[test]
fn a_message_sent_before_the_session_is_ready_waits_for_its_model_and_mode() {
    let request = OpenRequest { model: Some("fast".into()), mode: Some(PermissionMode::Plan), ..OpenRequest::default() };
    let mut run = Run::new(Goal::Open(request));
    run.command(Command::send("plan it"));
    run.initialized(json!({}));
    run.agent(respond(1, json!({
        "sessionId": "s1",
        "modes": { "currentModeId": "agent", "availableModes": [] },
        "configOptions": [{ "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "auto", "options": [] }],
    })));
    assert!(run.sent("session/prompt").is_empty(), "the model and mode are not in force yet");
    run.agent(respond(2, json!({})));
    assert!(run.sent("session/prompt").is_empty(), "the mode is not in force yet");
    run.agent(fail(3, -32602, "no plan mode"));
    assert_eq!(run.sent("session/prompt").len(), 1, "both answered, one refused: the message goes");
}

#[test]
fn a_model_and_mode_already_in_force_are_not_set_again() {
    let request = OpenRequest { model: Some("auto".into()), mode: Some(PermissionMode::Ask), ..OpenRequest::default() };
    let mut run = Run::new(Goal::Open(request));
    run.initialized(json!({}));
    run.agent(respond(1, json!({
        "sessionId": "s1",
        "modes": { "currentModeId": "agent", "availableModes": [] },
        "configOptions": [{ "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "auto", "options": [] }],
    })));
    assert!(run.sent("session/set_config_option").is_empty() && run.sent("session/set_mode").is_empty());
}

#[test]
fn a_mode_the_agent_has_no_name_for_is_unsupported() {
    let mut run = Run::ready();
    assert!(matches!(run.protocol.command(Command::SetPermissionMode { mode: PermissionMode::Bypass }, std::time::Instant::now()), Err(SessionError::Unsupported(_))));
    run.command(Command::SetPermissionMode { mode: PermissionMode::Plan });
    assert_eq!(run.last()["params"], json!({ "sessionId": "s1", "modeId": "plan" }));
}

#[test]
fn a_change_the_agent_refuses_is_a_warning() {
    let mut run = Run::ready();
    run.events();
    run.command(Command::SetModel { model: "nope".into() });
    run.agent(fail(2, -32602, "no such model"));
    assert_eq!(run.events(), vec![Event::Warning("the agent kept its model: no such model".into())]);
}

#[test]
fn the_agents_own_changes_of_mode_model_and_commands_say_started_again() {
    let mut run = Run::ready();
    run.events();
    run.agent(update(json!({ "sessionUpdate": "current_mode_update", "currentModeId": "plan" })));
    run.agent(update(json!({ "sessionUpdate": "available_commands_update", "availableCommands": [{ "name": "review", "description": "" }] })));
    run.agent(update(json!({ "sessionUpdate": "config_option_update", "configOptions": [{ "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "fast", "options": [] }] })));
    assert_eq!(run.events(), vec![
        started(Some("auto"), Some(PermissionMode::Plan), &[]),
        started(Some("auto"), Some(PermissionMode::Plan), &["review"]),
        started(Some("fast"), Some(PermissionMode::Plan), &["review"]),
    ]);
}

#[test]
fn commands_named_before_the_session_starts_are_in_its_start() {
    let mut run = Run::open();
    run.initialized(json!({}));
    run.agent(update(json!({ "sessionUpdate": "available_commands_update", "availableCommands": [{ "name": "plan", "description": "" }] })));
    assert!(run.events().is_empty());
    run.agent(respond(1, json!({ "sessionId": "s1" })));
    assert_eq!(run.events(), vec![started(None, None, &["plan"])]);
}

#[test]
fn an_agent_that_exits_mid_turn_fails_the_turn_with_its_last_words() {
    let mut run = Run::ready();
    run.command(Command::send("go"));
    run.agent(update(json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "ls", "kind": "execute", "status": "in_progress" })));
    run.events();
    let events = run.protocol.exited(Some(3), "starting\nno network\n\n", run.start);
    assert!(matches!(&events[0], Event::ToolFinished { output, .. } if output.is_error));
    assert!(matches!(&events[1], Event::TurnEnded(end) if end.outcome == TurnOutcome::Failed("the agent exited with code 3: no network".into())));
    assert_eq!(events[2], Event::Ended(EndReason::Exited { code: Some(3), stderr: "starting\nno network".into() }));
    assert!(run.protocol.closed().is_empty(), "a session ends once");
}

#[test]
fn a_session_atelier_closes_ends_once() {
    let mut run = Run::ready();
    assert_eq!(run.protocol.closed(), vec![Event::Ended(EndReason::Closed)]);
    assert!(run.protocol.closed().is_empty());
    assert!(run.protocol.exited(Some(0), "", run.start).is_empty());
    assert!(matches!(run.protocol.command(Command::send("late"), std::time::Instant::now()), Err(SessionError::Closed)));
}

#[test]
fn a_line_that_does_not_parse_is_a_warning() {
    let mut run = Run::ready();
    run.events();
    run.protocol.line("not json", run.start);
    let step = run.protocol.line("{oops", run.start);
    assert!(matches!(step.events.as_slice(), [Event::Warning(text)] if text.starts_with("a line from the agent did not parse")));
}

#[test]
fn a_list_asks_for_the_projects_sessions_and_shows_them_newest_first() {
    let mut run = Run::new(Goal::List);
    run.initialized(json!({ "sessionCapabilities": { "list": {} } }));
    assert_eq!(run.last()["method"], "session/list");
    assert_eq!(run.last()["params"], json!({ "cwd": "/work/project" }));
    let long = "x".repeat(300);
    run.agent(respond(1, json!({ "sessions": [
        { "sessionId": "a", "cwd": "/work/project", "title": "Older", "updatedAt": "2026-09-01T00:00:00Z" },
        { "sessionId": "b", "cwd": "/work/project", "title": long, "updatedAt": "2026-10-01T00:00:00Z" },
        { "sessionId": "c", "cwd": "/work/project", "title": "  " },
    ] })));
    assert!(run.done);
    let Some(Ok(Found::Sessions(rows))) = run.protocol.found() else { panic!("sessions") };
    let ids: Vec<_> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["b", "a", "c"], "newest first; no time goes last");
    assert_eq!(rows[0].title.chars().count(), 100, "a long title is cut");
    assert_eq!(rows[2].title, "Untitled session");
    assert_eq!(rows[1].updated, Some(1_788_220_800));
}

#[test]
fn an_agent_with_no_list_says_it_has_none() {
    let mut run = Run::new(Goal::List);
    run.initialized(json!({}));
    assert!(matches!(run.protocol.found(), Some(Err(SessionError::Unsupported(_)))));
}

#[test]
fn a_list_signs_in_when_the_agent_asks() {
    let mut run = Run::new(Goal::List);
    run.initialized(json!({ "sessionCapabilities": { "list": {} } }));
    run.agent(fail(1, -32000, "Authentication required"));
    assert_eq!(run.last()["method"], "authenticate");
    run.agent(respond(2, json!({})));
    assert_eq!(run.sent("session/list").len(), 2);
}

#[test]
fn a_history_is_what_the_agent_replays_as_it_loads_the_session() {
    let mut run = Run::new(Goal::History(SessionId::new("old")));
    run.initialized(json!({ "loadSession": true }));
    assert_eq!(run.last()["method"], "session/load");
    let said = |text: &str| update_in("old", json!({ "sessionUpdate": "user_message_chunk", "messageId": "m1", "content": { "type": "text", "text": text } }));
    run.agent(said("fix the "));
    run.agent(said("test"));
    run.agent(chunk_in("old", "agent_message_chunk", "On it."));
    run.agent(update_in("old", json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "cargo test", "kind": "execute", "status": "in_progress" })));
    assert!(run.events().is_empty(), "a history gives no live events");
    run.agent(respond(1, json!(null)));
    let Some(Ok(Found::History(events))) = run.protocol.found() else { panic!("a history") };
    assert_eq!(events[0], Event::UserMessage { text: "fix the test".into() }, "a user message's chunks are joined");
    assert!(matches!(&events[1], Event::Text { delta, .. } if delta == "On it."));
    assert!(matches!(&events[2], Event::ToolStarted(_)));
    assert!(matches!(&events[3], Event::ToolFinished { output, .. } if !output.is_error), "a call the record leaves open ends");
}

#[test]
fn user_chunks_with_no_message_id_are_messages_apart() {
    let mut run = Run::new(Goal::History(SessionId::new("old")));
    run.initialized(json!({ "loadSession": true }));
    run.agent(chunk_in("old", "user_message_chunk", "first"));
    run.agent(chunk_in("old", "user_message_chunk", "second"));
    run.agent(respond(1, json!(null)));
    let Some(Ok(Found::History(events))) = run.protocol.found() else { panic!("a history") };
    assert_eq!(events, [Event::UserMessage { text: "first".into() }, Event::UserMessage { text: "second".into() }]);
}

#[test]
fn a_history_that_cannot_load_is_an_error() {
    let mut run = Run::new(Goal::History(SessionId::new("old")));
    run.initialized(json!({ "loadSession": true }));
    run.agent(fail(1, -32602, "no such session"));
    assert!(matches!(run.protocol.found(), Some(Err(SessionError::Start(why))) if why == "no such session"));
}

/// A model and a mode asked for one after the other: the turn waits for both answers, whichever comes first,
/// so it never runs on the setting of an older request.
#[test]
fn a_turn_waits_for_every_setting_the_user_asked_for() {
    let mut run = Run::ready();
    run.command(Command::SetModel { model: "fast".into() });
    run.command(Command::SetPermissionMode { mode: PermissionMode::Plan });
    run.command(Command::send("go"));
    assert!(run.sent("session/prompt").is_empty(), "the settings are not answered");
    run.agent(respond(3, json!(null)));
    assert!(run.sent("session/prompt").is_empty(), "one setting is still open");
    run.agent(respond(2, json!(null)));
    assert_eq!(run.sent("session/prompt").len(), 1);
}

/// An id that comes back as text, `"2"` for 2, answers the request.
#[test]
fn an_answer_whose_id_is_text_still_answers_a_setting() {
    let mut run = Run::ready();
    run.command(Command::SetModel { model: "fast".into() });
    run.command(Command::send("go"));
    run.agent(json!({ "jsonrpc": "2.0", "id": "2", "result": null }));
    assert_eq!(run.sent("session/prompt").len(), 1);
}

/// An agent that never answers a setting must not hold the user's messages for good: after ten seconds the
/// next message goes out, on the setting the agent last said.
#[test]
fn a_setting_the_agent_never_answers_stops_holding_the_turn() {
    let mut run = Run::ready();
    run.command(Command::SetModel { model: "fast".into() });
    run.command(Command::send("go"));
    assert!(run.sent("session/prompt").is_empty());
    run.wait(30_000).command(Command::send("again"));
    assert_eq!(run.sent("session/prompt").len(), 1, "the held message goes out");
    assert_eq!(run.sent("session/prompt")[0]["params"]["prompt"][0]["text"], "go");
}
