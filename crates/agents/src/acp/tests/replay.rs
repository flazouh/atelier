//! Captured runs of Cursor's `agent acp` (2026.10.01-14929f9) from `tests/fixtures/cursor`, replayed through
//! the protocol with Cursor's own settings. Each fixture holds both sides: what the agent wrote, and what the
//! client that drove it wrote, which the replay turns back into atelier's commands. atelier must send the same
//! requests, and the events must read right.
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    acp::protocol::{Found, Goal, Protocol},
    cursor,
    session::{
        ChoiceId, ChoiceKind, Command, Event, OpenRequest, PermissionMode, RequestId, SessionError, SessionId, ToolKind,
        TurnOutcome,
    },
};

struct Replay {
    /// The requests and notifications atelier wrote, and those the capture sent, each as its method and params.
    written: Vec<Value>,
    captured: Vec<Value>,
    /// Every line atelier wrote.
    lines: Vec<Value>,
    events: Vec<Event>,
    found: Option<Result<Found, SessionError>>,
}

fn replay(name: &str, goal: Goal) -> Replay {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "cursor", &format!("{name}.jsonl")].iter().collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let (mut protocol, first) = Protocol::new(Arc::new(cursor::agent()), "/work/project".into(), goal);
    let mut lines: Vec<Value> = first.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (mut events, mut captured) = (Vec::new(), Vec::new());
    let start = Instant::now();
    for (i, line) in text.lines().enumerate() {
        let entry: Value = serde_json::from_str(line).unwrap();
        let message = &entry["line"];
        let step = if entry["dir"] == "in" {
            protocol.line(&message.to_string(), start + Duration::from_millis(i as u64))
        } else {
            if message["method"].is_string() {
                captured.push(request(message));
            }
            match command(message) {
                Some(command) => protocol.command(command, start + Duration::from_millis(i as u64)).expect("the command is taken"),
                None => continue,
            }
        };
        lines.extend(step.lines.iter().map(|l| serde_json::from_str::<Value>(l).unwrap()));
        events.extend(step.events);
    }
    let written = lines.iter().filter(|l| l["method"].is_string()).map(request).collect();
    Replay { written, captured, lines, events, found: protocol.found() }
}

fn request(line: &Value) -> Value {
    serde_json::json!({ "method": line["method"], "params": line["params"] })
}

/// The command of atelier's that writes what the capture's client wrote. The handshake and the answers to
/// the agent's own requests atelier writes by itself.
fn command(message: &Value) -> Option<Command> {
    let params = &message["params"];
    let text = |key: &str| params[key].as_str().unwrap_or_default().to_string();
    match message["method"].as_str() {
        Some("session/prompt") => Some(Command::send(params["prompt"][0]["text"].as_str().unwrap_or_default())),
        Some("session/set_model") => Some(Command::SetModel { model: text("modelId") }),
        Some("session/set_mode") => Some(Command::SetPermissionMode { mode: if text("modeId") == "plan" { PermissionMode::Plan } else { PermissionMode::Ask } }),
        Some("session/cancel") => Some(Command::Interrupt),
        Some(_) => None,
        None => {
            let choice = message["result"]["outcome"]["optionId"].as_str()?;
            let request = match &message["id"] {
                Value::String(id) => id.clone(),
                id => id.to_string(),
            };
            Some(Command::Answer { request: RequestId::new(request), choice: ChoiceId::new(choice) })
        }
    }
}

fn outcomes(events: &[Event]) -> Vec<TurnOutcome> {
    events.iter().filter_map(|e| if let Event::TurnEnded(end) = e { Some(end.outcome.clone()) } else { None }).collect()
}

fn summaries(events: &[Event]) -> Vec<Option<String>> {
    events.iter().filter_map(|e| if let Event::TurnEnded(end) = e { Some(end.summary.clone()) } else { None }).collect()
}

#[test]
fn a_plain_reply_and_a_read_replay_with_thinking_text_and_the_files_contents() {
    let run = replay("plain_and_read", Goal::Open(OpenRequest::default()));
    assert_eq!(run.written, run.captured, "atelier sends what the capture sent");
    let Event::Started(started) = &run.events[0] else { panic!("the session starts first") };
    assert_eq!(started.session.as_str(), "3857fa36-3630-4b23-bc28-8b6c401955ce");
    assert_eq!(started.model.as_deref(), Some("default"), "Cursor's `default[]` by its name");
    assert_eq!(started.mode, Some(PermissionMode::Ask), "Cursor's `agent` mode");
    assert!(run.events.iter().any(|e| matches!(e, Event::Thinking { delta, .. } if delta.contains("reply containing exactly"))));
    assert!(run.events.iter().any(|e| matches!(e, Event::ThinkingDone { .. })));

    let read = run.events.iter().find_map(|e| if let Event::ToolStarted(call) = e { Some(call) } else { None }).expect("a read");
    assert_eq!((read.name.as_str(), read.kind), ("Read File", ToolKind::Read));
    assert!(run.events.iter().any(|e| matches!(e, Event::ToolInput { input, file, .. }
        if input["path"] == "/work/project/main.rs" && file.as_deref() == Some("/work/project/main.rs"))));
    assert!(run.events.iter().any(|e| matches!(e, Event::ToolFinished { output, .. }
        if output.text == "fn main() {\n    println!(\"hello\");\n}\n" && !output.is_error)), "the raw output's content is the text");

    assert_eq!(outcomes(&run.events), [TurnOutcome::Completed, TurnOutcome::Completed]);
    assert_eq!(summaries(&run.events), [Some("pong".into()), Some("It prints `hello`.".into())]);
}

#[test]
fn edits_commands_questions_plan_mode_and_an_interrupt_replay() {
    let run = replay("session", Goal::Open(OpenRequest::default()));
    assert_eq!(run.written, run.captured, "atelier sends what the capture sent");

    let finished = |text: &str| run.events.iter().any(|e| matches!(e, Event::ToolFinished { output, .. } if output.text == text));
    assert!(finished("Changed /work/project/main.rs"), "an edit");
    assert!(finished("Created /work/project/notes.txt"), "Cursor marks a new file with `-- /dev/null`");
    assert!(run.events.iter().any(|e| matches!(e, Event::ToolFinished { output, .. } if output.text.starts_with("total 8\n"))), "a command's stdout");

    let questions: Vec<_> = run.events.iter().filter_map(|e| if let Event::Permission(r) = e { Some(r) } else { None }).collect();
    assert_eq!(questions.len(), 3, "rm, the plan's ls, and the sleep");
    assert_eq!(questions[0].reason.as_deref(), Some("Not in allowlist: rm"));
    assert_eq!(questions[0].call.input["command"], "rm notes.txt");
    assert_eq!(questions[0].choices.iter().map(|c| c.kind).collect::<Vec<_>>(), [ChoiceKind::Allow, ChoiceKind::AllowAlways, ChoiceKind::Deny]);
    let started: Vec<_> = run.events.iter().filter_map(|e| if let Event::ToolStarted(call) = e { Some(&call.id) } else { None }).collect();
    assert_eq!(started.len(), started.iter().collect::<std::collections::HashSet<_>>().len(), "a question adds no second card");
    assert!(questions.iter().all(|q| started.contains(&&q.call.id)), "each question names a call already on screen");
    let rm = &questions[0].call.id;
    assert!(run.events.iter().any(|e| matches!(e, Event::ToolFinished { id, output } if id == rm && output.text.is_empty())),
        "a denied command ends with nothing, not with the question's reason");
    assert!(run.lines.iter().any(|l| l["id"] == 0 && l["result"]["outcome"]["optionId"] == "reject-once"), "the answer reaches the agent");

    assert!(run.events.iter().any(|e| matches!(e, Event::Started(s) if s.mode == Some(PermissionMode::Plan))), "plan mode");
    assert!(run.events.iter().any(|e| matches!(e, Event::Todos(todos) if todos.len() == 3)), "the plan is the todo list");
    assert!(run.lines.iter().any(|l| l["error"]["code"] == -32601), "Cursor's own `cursor/create_plan` is declined");

    assert!(run.events.iter().any(|e| matches!(e, Event::PermissionCancelled(id) if id.as_str() == "3")), "the interrupt withdraws the question");
    assert_eq!(outcomes(&run.events), [
        TurnOutcome::Completed,
        TurnOutcome::Completed,
        TurnOutcome::Completed,
        TurnOutcome::Completed,
        TurnOutcome::Completed,
        TurnOutcome::Interrupted,
        TurnOutcome::Completed,
    ]);
    assert!(!run.events.iter().any(|e| matches!(e, Event::Warning(_))), "nothing in the run is unknown to atelier");
}

#[test]
fn a_loaded_session_replays_as_history_with_each_user_message_apart() {
    let run = replay("load", Goal::History(SessionId::new("c1b556fd-34c1-43c8-bedb-3fb6bf257a95")));
    assert_eq!(run.written, run.captured);
    let Some(Ok(Found::History(events))) = run.found else { panic!("a history") };
    let said: Vec<_> = events.iter().filter_map(|e| if let Event::UserMessage { text } = e { Some(text.as_str()) } else { None }).collect();
    assert_eq!(said.len(), 7);
    assert_eq!(&said[5..], ["Run the shell command: sleep 30 && echo done", "Say: still here"], "two messages in a row stay two");
    let started = events.iter().filter(|e| matches!(e, Event::ToolStarted(_))).count();
    let ended = events.iter().filter(|e| matches!(e, Event::ToolFinished { .. })).count();
    assert_eq!(started, ended, "every replayed call ends");
}

#[test]
fn the_session_list_replays_newest_first() {
    let run = replay("list", Goal::List);
    assert_eq!(run.written, run.captured);
    let Some(Ok(Found::Sessions(rows))) = run.found else { panic!("sessions") };
    let rows: Vec<_> = rows.iter().map(|r| (r.title.as_str(), r.updated)).collect();
    assert_eq!(rows, [("Change Hello To Bonjour", Some(1_790_882_594)), ("Pong", Some(1_790_882_560))]);
}
