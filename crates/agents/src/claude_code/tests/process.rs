//! The live path, against a stand-in `claude`: a shell script that plays back a captured run or
//! records what lathe writes to it. Only the process is fake; the project, the threads and the
//! pipes are the real ones.
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use lathe_project::LocalProject;

use crate::{
    claude_code::ClaudeCode,
    session::{
        Attachment, Backend, Command, EndReason, Event, EventSink, OpenRequest, RequestId, ChoiceId, SessionError, TurnOutcome,
    },
};

const WAIT: Duration = Duration::from_secs(10);

struct Stand {
    dir: tempfile::TempDir,
}

impl Stand {
    fn new() -> Self {
        Self { dir: tempfile::tempdir().expect("a temp folder") }
    }

    fn script(&self, body: &str) -> PathBuf {
        let path = self.dir.path().join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn project(&self) -> Arc<dyn lathe_project::Project> {
        Arc::new(LocalProject::open(self.dir.path()).unwrap())
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

#[test]
fn a_program_the_host_lacks_is_reported_as_missing() {
    let stand = Stand::new();
    let backend = ClaudeCode::with_program("/nonexistent/lathe-test-claude");
    let (sink, _) = channel();
    match backend.open(stand.project(), OpenRequest::default(), sink) {
        Err(SessionError::Missing { program }) => assert!(program.contains("lathe-test-claude")),
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("a missing program started"),
    }
}

#[test]
fn a_played_back_run_becomes_events_and_ends_with_the_exit_code() {
    let stand = Stand::new();
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "claude_code", "plain.jsonl"].iter().collect();
    let program = stand.script(&format!("cat '{}'", fixture.display()));
    let (sink, rx) = channel();
    let _session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    let events = until(&rx, ended);
    assert!(matches!(events[0], Event::Started(_)));
    assert!(events.iter().any(|e| matches!(e, Event::TurnEnded(end) if end.outcome == TurnOutcome::Completed)));
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Exited { code: Some(0), stderr: String::new() })));
}

#[test]
fn the_process_dying_mid_turn_fails_the_turn_and_the_open_tool() {
    let stand = Stand::new();
    let program = stand.script(
        r#"read first
echo '{"type":"system","subtype":"init","session_id":"s1"}'
echo '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":"Bash","input":{}}}}'
exit 3"#,
    );
    let (sink, rx) = channel();
    let session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    session.send(Command::send("go")).unwrap();
    let events = until(&rx, ended);
    assert!(events.iter().any(|e| matches!(e, Event::ToolFinished { output, .. } if output.is_error)));
    assert!(events.iter().any(|e| matches!(e, Event::TurnEnded(end) if matches!(&end.outcome, TurnOutcome::Failed(why) if why.contains("code 3")))));
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Exited { code: Some(3), stderr: String::new() })));
}

#[test]
fn commands_reach_the_process_as_stream_json_lines() {
    let stand = Stand::new();
    let log = stand.dir.path().join("log");
    let program = stand.script(&format!(
        r#"echo '{{"type":"system","subtype":"init","session_id":"s1"}}'
echo '{{"type":"control_request","request_id":"r1","request":{{"subtype":"can_use_tool","tool_name":"Bash","input":{{"command":"ls"}},"tool_use_id":"t1"}}}}'
while IFS= read -r line; do echo "$line" >> '{}'; done"#,
        log.display()
    ));
    let (sink, rx) = channel();
    let session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    until(&rx, |e| matches!(e, Event::Permission(_)));
    session.send(Command::send("hello")).unwrap();
    session.send(Command::Answer { request: RequestId::new("r1"), choice: ChoiceId::new("allow") }).unwrap();
    session.send(Command::Interrupt).unwrap();
    let deadline = Instant::now() + WAIT;
    let written = loop {
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if text.lines().count() >= 3 || Instant::now() > deadline {
            break text;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let lines: Vec<serde_json::Value> = written.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines[0]["message"]["content"], "hello");
    assert_eq!(lines[1]["response"]["response"]["behavior"], "allow");
    assert_eq!(lines[2]["request"]["subtype"], "interrupt");
    // The request is answered; answering again finds nothing waiting.
    let again = session.send(Command::Answer { request: RequestId::new("r1"), choice: ChoiceId::new("allow") });
    assert!(matches!(again, Err(SessionError::Unsupported(_))));
}

#[test]
fn dropping_the_session_stops_the_process_and_ends_closed() {
    let stand = Stand::new();
    let program = stand.script("echo '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\"}'\nsleep 60");
    let (sink, rx) = channel();
    let session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    until(&rx, |e| matches!(e, Event::Started(_)));
    let dropped = Instant::now();
    drop(session);
    let events = until(&rx, ended);
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Closed)));
    assert!(dropped.elapsed() < Duration::from_secs(5), "the process must not run its sleep out");
}

#[test]
fn a_message_with_attachments_reaches_the_process_as_one_text() {
    let stand = Stand::new();
    let log = stand.dir.path().join("log");
    let program = stand.script(&format!(
        "echo '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\"}}'\nwhile IFS= read -r line; do printf '%s\\n' \"$line\" >> '{}'; done",
        log.display()
    ));
    let (sink, rx) = channel();
    let session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    until(&rx, |e| matches!(e, Event::Started(_)));
    let comment = Attachment::LineComment { path: "src/a.rs".into(), first_line: 3, last_line: 3, removed: false, quote: "let x = 1;".into(), body: "why?".into() };
    session.send(Command::Send { text: "look at this".into(), attachments: vec![comment] }).unwrap();
    let deadline = Instant::now() + WAIT;
    let written = loop {
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        if !text.is_empty() || Instant::now() > deadline {
            break text;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let line: serde_json::Value = serde_json::from_str(written.lines().next().unwrap()).unwrap();
    assert_eq!(line["message"]["content"], "look at this\n\nReview comment on src/a.rs, line 3:\n> let x = 1;\nwhy?");
}

#[test]
fn a_process_that_says_why_on_stderr_and_exits_gives_that_text_in_its_end_events() {
    let stand = Stand::new();
    let program = stand.script("read first\necho 'no such model' >&2\nexit 3");
    let (sink, rx) = channel();
    let session = ClaudeCode::with_program(program.to_string_lossy()).open(stand.project(), OpenRequest::default(), sink).unwrap();
    session.send(Command::send("go")).unwrap();
    let events = until(&rx, ended);
    assert!(events.iter().any(|e| matches!(e, Event::TurnEnded(end)
        if matches!(&end.outcome, TurnOutcome::Failed(why) if why == "the agent exited with code 3: no such model"))));
    assert_eq!(events.last(), Some(&Event::Ended(EndReason::Exited { code: Some(3), stderr: "no such model".into() })));
}
