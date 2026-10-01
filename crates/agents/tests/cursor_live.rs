//! Runs Cursor's real `agent acp` (2026.10.01-14929f9 or later, logged in with `agent login`) through the
//! ACP backend, end to end: a turn, a command that asks first and is denied, an interrupt, a model switch,
//! a resume, the session list and the history. It uses the account's requests and needs a network, so it
//! is ignored unless asked for:
//! `cargo test -p atelier-agents --test cursor_live -- --ignored --nocapture`
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_agents::{
    acp::Acp,
    cursor,
    session::{
        Backend, ChoiceKind, Command, EndReason, Event, EventSink, OpenRequest, PermissionMode, Session, SessionId,
        TurnOutcome,
    },
};
use atelier_project::{LocalProject, Project};

const WAIT: Duration = Duration::from_secs(180);

fn open(project: &Arc<dyn Project>, request: OpenRequest) -> (Box<dyn Session>, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let sink: EventSink = Arc::new(move |event| drop(tx.lock().unwrap().send(event)));
    let session = Acp::new(cursor::agent()).open(project.clone(), request, sink).expect("agent acp starts");
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

fn said(events: &[Event], word: &str) -> bool {
    let text: String = events.iter().filter_map(|e| if let Event::Text { delta, .. } = e { Some(delta.as_str()) } else { None }).collect();
    text.to_lowercase().contains(word)
}

#[test]
#[ignore = "runs the real agent acp"]
fn a_real_cursor_session_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.txt"), "keep\n").unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let request = OpenRequest { model: Some("composer-2.5".into()), mode: Some(PermissionMode::Ask), ..OpenRequest::default() };
    let (session, rx) = open(&project, request);
    let mut seen = Vec::new();

    // 1. A plain turn, on the model asked for by its name.
    session.send(Command::send("Reply with the single word pong.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    let id: SessionId = seen.iter().find_map(|e| if let Event::Started(s) = e { Some(s.session.clone()) } else { None }).unwrap();
    assert!(seen.iter().any(|e| matches!(e, Event::Started(s) if s.model.as_deref() == Some("composer-2.5"))), "{seen:#?}");
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    assert!(said(&seen, "pong"));
    println!("plain turn: ok, session {}", id.as_str());

    // 2. A command outside the allowlist asks first; denied, it does not run.
    seen.clear();
    session.send(Command::send("Run the shell command: rm keep.txt")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_) | Event::TurnEnded(_)));
    let Some(Event::Permission(request)) = seen.last().cloned() else { panic!("no question: {seen:#?}") };
    assert!(request.reason.as_deref().is_some_and(|r| r.contains("rm")), "reason: {:?}", request.reason);
    let deny = request.choices.iter().find(|c| c.kind == ChoiceKind::Deny).unwrap().id.clone();
    session.send(Command::Answer { request: request.id, choice: deny }).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    assert!(dir.path().join("keep.txt").exists(), "a denied command ran");
    println!("permission denied: the file stays");

    // 3. A command interrupted while it waits for an answer.
    seen.clear();
    session.send(Command::send("Run the shell command: sleep 30 && echo done")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_) | Event::TurnEnded(_)));
    session.send(Command::Interrupt).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert_eq!(outcome(&seen), TurnOutcome::Interrupted);
    println!("interrupt: turn interrupted");

    // 4. A model switch shows as a new Started, and the turn after it runs.
    seen.clear();
    session.send(Command::SetModel { model: "default".into() }).unwrap();
    session.send(Command::send("Reply with the single word ok.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(seen.iter().any(|e| matches!(e, Event::Started(s) if s.model.as_deref() == Some("default"))));
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    println!("set model: the next turn runs on default");
    drop(session);
    until(&rx, &mut seen, |e| matches!(e, Event::Ended(EndReason::Closed)));

    // 5. Resume by id and ask what came before.
    let (resumed, rx) = open(&project, OpenRequest { resume: Some(id.clone()), ..OpenRequest::default() });
    let mut seen = Vec::new();
    resumed.send(Command::send("What single word did I first ask you to reply with? Answer with that word only.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(said(&seen, "pong"), "{seen:#?}");
    assert!(!seen.iter().any(|e| matches!(e, Event::UserMessage { .. })), "a resume does not repeat the history");
    println!("resume: the session remembers");
    drop(resumed);

    // 6. The list and the history.
    let backend = Acp::new(cursor::agent());
    let sessions = backend.sessions(project.as_ref()).unwrap();
    assert!(sessions.iter().any(|s| s.id == id), "the session is listed: {sessions:#?}");
    let history = backend.history(project.as_ref(), &id).unwrap();
    assert!(history.iter().any(|e| matches!(e, Event::UserMessage { text } if text.contains("rm keep.txt"))));
    assert!(history.iter().any(|e| matches!(e, Event::ToolStarted(_))));
    println!("list: {} session(s); history: {} events", sessions.len(), history.len());

    // 7. A missing program says so.
    let (tx, _rx) = mpsc::channel::<Event>();
    let tx = Mutex::new(tx);
    let sink: EventSink = Arc::new(move |event| drop(tx.lock().unwrap().send(event)));
    assert!(Acp::new(cursor::agent().with_program("/no/such/agent")).open(project, OpenRequest::default(), sink).is_err());
}
