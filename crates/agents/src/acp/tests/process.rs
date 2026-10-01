//! The live path, against a stand-in agent: a shell script that answers atelier's requests in order (atelier
//! numbers them from 0) or records what atelier writes. Only the agent is fake; the project, the threads
//! and the pipes are the real ones.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_project::LocalProject;

use super::agent;
use crate::{
    acp::Acp,
    session::{Backend, Command, EndReason, Event, EventSink, OpenRequest, SessionError, SessionId, TurnOutcome},
};

const WAIT: Duration = Duration::from_secs(10);

const INITIALIZED: &str =
    r#"{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"sessionCapabilities":{"list":{}}},"authMethods":[]}}"#;

struct Stand {
    dir: tempfile::TempDir,
}

impl Stand {
    fn new() -> Self {
        Self { dir: tempfile::tempdir().expect("a temp folder") }
    }

    /// A backend whose agent is `body`, a shell script.
    fn backend(&self, body: &str) -> Acp {
        let path = self.dir.path().join("agent");
        crate::testing::write_script(&path, &format!("#!/bin/sh\n{body}\n"));
        Acp::new(agent().with_program(path.to_string_lossy()))
    }

    fn project(&self) -> Arc<dyn atelier_project::Project> {
        Arc::new(crate::testing::Locked(Arc::new(LocalProject::open(self.dir.path()).unwrap())))
    }

    fn log(&self) -> PathBuf {
        self.dir.path().join("log")
    }
}

fn channel() -> (EventSink, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    (Arc::new(move |event| drop(tx.lock().unwrap().send(event))), rx)
}

/// Events until one matches, or the wait runs out.
fn until(rx: &mpsc::Receiver<Event>, done: impl Fn(&Event) -> bool) -> Vec<Event> {
    let deadline = Instant::now() + WAIT;
    let mut seen = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = rx.recv_timeout(left) else { break };
        let stop = done(&event);
        seen.push(event);
        if stop {
            return seen;
        }
    }
    panic!("the wait ran out; saw {seen:#?}");
}

fn ended(event: &Event) -> bool {
    matches!(event, Event::Ended(_))
}

/// The lines in the log once it has `count` of them, or the wait runs out.
fn logged(path: &PathBuf, count: usize) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + WAIT;
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.lines().count() >= count || Instant::now() > deadline {
            return text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_program_the_host_lacks_is_reported_as_missing() {
    let stand = Stand::new();
    let backend = Acp::new(agent().with_program("/nonexistent/atelier-test-acp"));
    let (sink, _) = channel();
    match backend.open(stand.project(), OpenRequest::default(), sink) {
        Err(SessionError::Missing { program }) => assert!(program.contains("atelier-test-acp")),
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("a missing program started"),
    }
    assert!(matches!(backend.sessions(stand.project().as_ref()), Err(SessionError::Missing { .. })));
}

#[test]
fn a_turn_runs_through_the_process_and_atelier_writes_json_rpc_lines() {
    let stand = Stand::new();
    let log = stand.log();
    let backend = stand.backend(&format!(
        r#"log='{}'
read l; printf '%s\n' "$l" >> "$log"; echo '{INITIALIZED}'
read l; printf '%s\n' "$l" >> "$log"; echo '{{"jsonrpc":"2.0","id":1,"result":{{"sessionId":"s1"}}}}'
read l; printf '%s\n' "$l" >> "$log"
echo '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"s1","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"Hi there"}}}}}}}}'
echo '{{"jsonrpc":"2.0","id":2,"result":{{"stopReason":"end_turn"}}}}'
sleep 60"#,
        log.display()
    ));
    let (sink, rx) = channel();
    let session = backend.open(stand.project(), OpenRequest::default(), sink).unwrap();
    session.send(Command::send("hello")).unwrap();
    let events = until(&rx, |e| matches!(e, Event::TurnEnded(_)));
    assert!(matches!(&events[0], Event::Started(s) if s.session.as_str() == "s1"));
    assert!(events.iter().any(|e| matches!(e, Event::Text { delta, .. } if delta == "Hi there")));
    assert!(matches!(events.last(), Some(Event::TurnEnded(end)) if end.outcome == TurnOutcome::Completed));

    let lines = logged(&log, 3);
    assert_eq!(lines[0]["method"], "initialize");
    assert_eq!(lines[1]["method"], "session/new");
    assert_eq!(lines[1]["params"]["cwd"], stand.dir.path().canonicalize().unwrap().display().to_string());
    assert_eq!(lines[2]["method"], "session/prompt");
    assert_eq!(lines[2]["params"]["prompt"][0]["text"], "hello");

    let dropped = Instant::now();
    drop(session);
    assert_eq!(until(&rx, ended).last(), Some(&Event::Ended(EndReason::Closed)));
    assert!(dropped.elapsed() < Duration::from_secs(5), "the process must not run its sleep out");
}

#[test]
fn an_agent_that_dies_mid_turn_fails_the_turn_with_what_it_said_on_stderr() {
    let stand = Stand::new();
    let backend = stand.backend(&format!(
        r#"read l; echo '{INITIALIZED}'
read l; echo '{{"jsonrpc":"2.0","id":1,"result":{{"sessionId":"s1"}}}}'
read l; echo 'lost the connection' >&2; exit 4"#
    ));
    let (sink, rx) = channel();
    let session = backend.open(stand.project(), OpenRequest::default(), sink).unwrap();
    session.send(Command::send("go")).unwrap();
    let events = until(&rx, ended);
    assert!(events.iter().any(|e| matches!(e, Event::TurnEnded(end)
        if end.outcome == TurnOutcome::Failed("the agent exited with code 4: lost the connection".into()))));
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Exited { code: Some(4), stderr: "lost the connection".into() })));
}

#[test]
fn an_agent_that_cannot_start_a_session_is_stopped_with_its_reason() {
    let stand = Stand::new();
    let backend = stand.backend(&format!(
        r#"read l; echo '{INITIALIZED}'
read l; echo '{{"jsonrpc":"2.0","id":1,"error":{{"code":-32603,"message":"no workspace"}}}}'
sleep 60"#
    ));
    let (sink, rx) = channel();
    let _session = backend.open(stand.project(), OpenRequest::default(), sink).unwrap();
    let started = Instant::now();
    assert_eq!(until(&rx, ended).last(), Some(&Event::Ended(EndReason::Failed("no workspace".into()))));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn past_sessions_are_listed_by_asking_the_agent() {
    let stand = Stand::new();
    let backend = stand.backend(&format!(
        r#"read l; echo '{INITIALIZED}'
read l; echo '{{"jsonrpc":"2.0","id":1,"result":{{"sessions":[{{"sessionId":"a","cwd":"/p","title":"Fix the build","updatedAt":"2026-10-01T00:00:00Z"}}]}}}}'
sleep 60"#
    ));
    let started = Instant::now();
    let rows = backend.sessions(stand.project().as_ref()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].id.as_str(), rows[0].title.as_str()), ("a", "Fix the build"));
    assert!(started.elapsed() < Duration::from_secs(5), "the agent is stopped once it has answered");
}

#[test]
fn a_past_sessions_history_is_what_the_agent_replays() {
    let stand = Stand::new();
    let backend = stand.backend(&format!(
        r#"read l; echo '{INITIALIZED}'
read l
echo '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"a","update":{{"sessionUpdate":"user_message_chunk","content":{{"type":"text","text":"hello"}}}}}}}}'
echo '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"a","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"hi"}}}}}}}}'
echo '{{"jsonrpc":"2.0","id":1,"result":null}}'
sleep 60"#
    ));
    let events = backend.history(stand.project().as_ref(), &SessionId::new("a")).unwrap();
    assert_eq!(events[0], Event::UserMessage { text: "hello".into() });
    assert!(matches!(&events[1], Event::Text { delta, .. } if delta == "hi"));
}

#[test]
fn a_list_from_an_agent_that_exits_says_why() {
    let stand = Stand::new();
    let backend = stand.backend("read l; echo 'not signed in' >&2; exit 1");
    match backend.sessions(stand.project().as_ref()) {
        Err(SessionError::Read(why)) => assert_eq!(why, "the agent exited with code 1: not signed in"),
        other => panic!("wrong answer: {other:?}"),
    }
}
