//! Runs the real `claude` (2.1.284 or later, logged in) through the backend, end to end: a turn, a
//! permission question answered, an interrupt, a resume, the session list and the history. It costs a
//! few cents and needs a network, so it is ignored unless asked for:
//! `cargo test -p atelier-agents --test live -- --ignored --nocapture`
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_agents::{
    claude_code::ClaudeCode,
    session::{
        Backend, ChoiceKind, Command, EndReason, Event, EventSink, OpenRequest, PermissionMode, Session, SessionId,
        TurnOutcome,
    },
};
use atelier_project::{LocalProject, Project};

const WAIT: Duration = Duration::from_secs(120);

fn open(project: &Arc<dyn Project>, request: OpenRequest) -> (Box<dyn Session>, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let sink: EventSink = Arc::new(move |event| drop(tx.lock().unwrap().send(event)));
    let session = ClaudeCode::new().open(project.clone(), request, sink).expect("claude starts");
    (session, rx)
}

fn until(rx: &mpsc::Receiver<Event>, seen: &mut Vec<Event>, done: impl Fn(&Event) -> bool) {
    let deadline = Instant::now() + WAIT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = rx.recv_timeout(left) else { break };
        let stop = done(&event);
        seen.push(event);
        if stop {
            return;
        }
    }
    panic!("the wait ran out; saw {seen:#?}");
}

fn turn_ended(event: &Event) -> bool {
    matches!(event, Event::TurnEnded(_))
}

fn outcome(events: &[Event]) -> TurnOutcome {
    events.iter().rev().find_map(|e| if let Event::TurnEnded(end) = e { Some(end.outcome.clone()) } else { None }).unwrap()
}

/// The backend's permission questions rest on a flag that `claude --help` does not list,
/// `--permission-prompt-tool stdio` (seen in 2.1.284). Without it the CLI denies a write by itself and
/// sends no `control_request`. If a newer CLI drops or renames the flag, this test says so.
#[test]
#[ignore = "runs the real claude"]
fn the_cli_still_asks_for_permissions_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let request = OpenRequest { model: Some("haiku".into()), mode: Some(PermissionMode::Ask), ..OpenRequest::default() };
    let (session, rx) = open(&project, request);
    session.send(Command::send("Create a file named asked.txt containing hi, with the Write tool.")).unwrap();
    let mut seen = Vec::new();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_) | Event::TurnEnded(_)));
    assert!(
        matches!(seen.last(), Some(Event::Permission(_))),
        "claude did not send a control_request for a write. The hidden flag --permission-prompt-tool stdio \
         may be gone or renamed in this claude version; see crates/agents/src/claude_code/launch.rs. Saw: {seen:#?}"
    );
    assert!(!dir.path().join("asked.txt").exists(), "the write ran before anyone answered");
}

#[test]
#[ignore = "runs the real claude"]
fn a_real_session_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let request = OpenRequest { model: Some("haiku".into()), mode: Some(PermissionMode::Ask), ..OpenRequest::default() };
    let (session, rx) = open(&project, request);
    let mut seen = Vec::new();

    // 1. A plain turn.
    session.send(Command::send("Reply with the single word ok.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    let Event::Started(started) = &seen[0] else { panic!("first event: {:?}", seen[0]) };
    let id: SessionId = started.session.clone();
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    assert!(seen.iter().any(|e| matches!(e, Event::Text { delta, .. } if delta.to_lowercase().contains("ok"))));
    println!("plain turn: ok, session {}", id.as_str());

    // 2. A write that asks first, answered by us.
    seen.clear();
    session.send(Command::send("Create a file named made.txt containing hi, with the Write tool.")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_)));
    let Some(Event::Permission(request)) = seen.last().cloned() else { unreachable!() };
    assert_eq!(request.call.name, "Write");
    assert!(request.call.file.as_deref().is_some_and(|f| f.ends_with("made.txt")));
    let allow = request.choices.iter().find(|c| c.kind == ChoiceKind::Allow).unwrap().id.clone();
    session.send(Command::Answer { request: request.id, choice: allow }).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    assert_eq!(std::fs::read_to_string(dir.path().join("made.txt")).unwrap().trim(), "hi");
    println!("permission allowed: file written");

    // 3. A write that we interrupt while it waits for an answer.
    seen.clear();
    session.send(Command::send("Create a file named stop.txt containing hi, with the Write tool.")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_)));
    session.send(Command::Interrupt).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert_eq!(outcome(&seen), TurnOutcome::Interrupted);
    assert!(!dir.path().join("stop.txt").exists());
    println!("interrupt: turn interrupted, nothing written");

    // 4. Close, then resume by id and ask what came before.
    drop(session);
    let (resumed, rx) = open(&project, OpenRequest { resume: Some(id.clone()), model: Some("haiku".into()), ..OpenRequest::default() });
    let mut seen = Vec::new();
    resumed.send(Command::send("What single word did I first ask you to reply with? Answer with that word only.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(seen.iter().any(|e| matches!(e, Event::Text { delta, .. } if delta.to_lowercase().contains("ok"))));
    println!("resume: the session remembers");

    // 4b. A model switch shows as a new Started with the new model, and the turn still runs.
    seen.clear();
    resumed.send(Command::SetModel { model: "sonnet".into() }).unwrap();
    resumed.send(Command::SetPermissionMode { mode: PermissionMode::AcceptEdits }).unwrap();
    resumed.send(Command::send("Reply with the single word ok.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(seen.iter().any(|e| matches!(e, Event::Started(s) if s.model.as_deref().is_some_and(|m| m.contains("sonnet")))));
    println!("set model: the next turn runs on sonnet");
    drop(resumed);
    until(&rx, &mut seen, |e| matches!(e, Event::Ended(EndReason::Closed)));

    // 5. The list and the history.
    let backend = ClaudeCode::new();
    let sessions = backend.sessions(project.as_ref()).unwrap();
    let mine = sessions.iter().find(|s| s.id == id).expect("the session is listed");
    assert!(mine.title.contains("single word ok"), "title: {}", mine.title);
    let history = backend.history(project.as_ref(), &id).unwrap();
    assert!(history.iter().any(|e| matches!(e, Event::UserMessage { text } if text.contains("made.txt"))));
    assert!(history.iter().any(|e| matches!(e, Event::ToolStarted(call) if call.name == "Write")));
    println!("list: {} session(s); history: {} events", sessions.len(), history.len());

    // 6. A session in a folder without a record has no sessions, and a missing program says so.
    let fresh = tempfile::tempdir().unwrap();
    let empty: Arc<dyn Project> = Arc::new(LocalProject::open(fresh.path()).unwrap());
    assert!(backend.sessions(empty.as_ref()).unwrap().is_empty());
    let (tx, _rx) = mpsc::channel::<Event>();
    let tx = Mutex::new(tx);
    let sink: EventSink = Arc::new(move |event| drop(tx.lock().unwrap().send(event)));
    assert!(ClaudeCode::with_program("/no/such/claude").open(project, OpenRequest::default(), sink).is_err());
}

/// A run with no sign-in tells `SignedOut` and shows none of `claude`'s advice to run `/login`, which a headless run
/// cannot. It needs the `claude` program, not a network or an account: nothing reaches the API.
#[test]
#[ignore = "runs the real claude"]
fn a_run_with_no_sign_in_tells_it_and_fails_its_turn() {
    let config = tempfile::tempdir().unwrap();
    // SAFETY: this test is the only one that runs in its process.
    unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", config.path()) };
    let dir = tempfile::tempdir().unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let (session, rx) = open(&project, OpenRequest::default());
    session.send(Command::send("hi")).unwrap();
    let mut seen = Vec::new();
    until(&rx, &mut seen, turn_ended);
    assert!(seen.contains(&Event::SignedOut), "{seen:#?}");
    assert!(!seen.iter().any(|e| matches!(e, Event::Text { .. })), "the CLI's advice is not shown: {seen:#?}");
    assert!(matches!(outcome(&seen), TurnOutcome::Failed(_)));
}
