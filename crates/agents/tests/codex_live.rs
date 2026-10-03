//! Runs the real Codex adapter (`npx -y @agentclientprotocol/codex-acp`, with Codex logged in by `codex login`) through
//! the ACP backend, end to end: a turn, a command that asks first and is denied, an interrupt, a resume, the session
//! list and the history. It uses the account's requests and needs a network and Node, so it is ignored unless asked for:
//! `cargo test -p atelier-agents --test codex_live -- --ignored --nocapture`
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_agents::{
    acp::Acp,
    codex::Codex,
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
    let session = Acp::new(Codex::agent()).open(project.clone(), request, sink).expect("agent acp starts");
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
#[ignore = "runs the real Codex adapter"]
fn a_real_codex_session_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.txt"), "keep\n").unwrap();
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let request = OpenRequest { model: Some("gpt-5.5".into()), mode: Some(PermissionMode::Ask), ..OpenRequest::default() };
    let (session, rx) = open(&project, request);
    let mut seen = Vec::new();

    // 1. A plain turn, on the model asked for by its name.
    session.send(Command::send("Reply with the single word pong.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    let id: SessionId = seen.iter().find_map(|e| if let Event::Started(s) = e { Some(s.session.clone()) } else { None }).unwrap();
    assert!(seen.iter().any(|e| matches!(e, Event::Started(s) if s.model.as_deref() == Some("gpt-5.5"))), "{seen:#?}");
    assert_eq!(outcome(&seen), TurnOutcome::Completed);
    assert!(said(&seen, "pong"));
    println!("plain turn: ok, session {}", id.as_str());

    // 2. A command that changes something asks first; denied, it does not run.
    seen.clear();
    session.send(Command::send("Run the shell command: rm keep.txt")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_) | Event::TurnEnded(_)));
    let Some(Event::Permission(request)) = seen.last().cloned() else { panic!("no question: {seen:#?}") };
    let deny = request.choices.iter().find(|c| c.kind == ChoiceKind::Deny).unwrap().id.clone();
    session.send(Command::Answer { request: request.id, choice: deny }).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(dir.path().join("keep.txt").exists(), "a denied command ran");
    println!("permission denied: the file stays");

    // 3. A command interrupted while it runs or waits.
    seen.clear();
    session.send(Command::send("Run the shell command: sleep 30 && echo done")).unwrap();
    until(&rx, &mut seen, |e| matches!(e, Event::Permission(_) | Event::ToolStarted(_) | Event::TurnEnded(_)));
    session.send(Command::Interrupt).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert_eq!(outcome(&seen), TurnOutcome::Interrupted);
    println!("interrupt: turn interrupted");
    drop(session);
    until(&rx, &mut seen, |e| matches!(e, Event::Ended(EndReason::Closed)));

    // 4. Resume by id and ask what came before.
    let (resumed, rx) = open(&project, OpenRequest { resume: Some(id.clone()), ..OpenRequest::default() });
    let mut seen = Vec::new();
    resumed.send(Command::send("What single word did I first ask you to reply with? Answer with that word only.")).unwrap();
    until(&rx, &mut seen, turn_ended);
    assert!(said(&seen, "pong"), "{seen:#?}");
    println!("resume: the session remembers");
    drop(resumed);

    // 5. The list and the history.
    let backend = Acp::new(Codex::agent());
    let sessions = backend.sessions(project.as_ref()).unwrap();
    assert!(sessions.iter().any(|s| s.id == id), "the session is listed: {sessions:#?}");
    let history = backend.history(project.as_ref(), &id).unwrap();
    assert!(history.iter().any(|e| matches!(e, Event::UserMessage { text } if text.contains("pong"))), "{history:#?}");
    println!("list: {} session(s); history: {} events", sessions.len(), history.len());
}
