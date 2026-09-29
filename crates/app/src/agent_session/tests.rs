use std::sync::{Arc, Mutex};

use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext};
use lathe_agents::session::{
    Backend, Capabilities, Choice, ChoiceId, EventSink, PermissionRequest, RequestId, Session, Started, ToolCall, ToolId,
    ToolKind, ToolStatus, TurnEnd, TurnOutcome,
};

use super::*;

/// A backend in the test's thread: each message plays the next scripted turn into the sink, and every
/// command is kept. `fail_first` makes the first open fail as a missing program does.
struct Fake {
    turns: Mutex<Vec<Vec<Event>>>,
    received: Arc<Mutex<Vec<Command>>>,
    fail_first: Mutex<bool>,
}

struct FakeSession {
    backend: Arc<Fake>,
    sink: EventSink,
}

impl Session for FakeSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        let turn = matches!(command, Command::Send { .. });
        self.backend.received.lock().unwrap().push(command);
        if turn {
            let next = { let mut t = self.backend.turns.lock().unwrap(); if t.is_empty() { Vec::new() } else { t.remove(0) } };
            next.into_iter().for_each(|e| (self.sink)(e));
        }
        Ok(())
    }
}

struct FakeBackend(Arc<Fake>);

impl Backend for FakeBackend {
    fn name(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn open(&self, _: Arc<dyn Project>, request: OpenRequest, sink: EventSink) -> Result<Box<dyn Session>, SessionError> {
        if std::mem::take(&mut *self.0.fail_first.lock().unwrap()) {
            return Err(SessionError::Missing { program: "fake".into() });
        }
        let id = request.resume.unwrap_or_else(|| SessionId::new("fake-1"));
        sink(Event::Started(Started { session: id, model: None, mode: None }));
        Ok(Box::new(FakeSession { backend: self.0.clone(), sink }))
    }
}

fn ask() -> PermissionRequest {
    PermissionRequest {
        id: RequestId::new("r1"),
        call: ToolCall {
            id: ToolId::new("t1"),
            name: "Write".into(),
            kind: ToolKind::Write,
            input: serde_json::json!({ "file_path": "a.txt" }),
            file: Some("a.txt".into()),
            parent: None,
            status: ToolStatus::Pending,
        },
        reason: None,
        choices: vec![
            Choice { id: ChoiceId::new("yes"), label: "Allow".into(), kind: ChoiceKind::Allow },
            Choice { id: ChoiceId::new("always"), label: "Always".into(), kind: ChoiceKind::AllowAlways },
            Choice { id: ChoiceId::new("no"), label: "Deny".into(), kind: ChoiceKind::Deny },
        ],
    }
}

fn ended() -> Event {
    Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Completed, summary: None })
}

fn start(cx: &mut TestAppContext, turns: Vec<Vec<Event>>, fail_first: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let fake = Arc::new(Fake { turns: Mutex::new(turns), received: Arc::default(), fail_first: Mutex::new(fail_first) });
    let dir = tempfile::tempdir().unwrap().keep();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let mut agent = lathe_agents::registry::agents().remove(0);
    agent.backend = Arc::new(FakeBackend(fake.clone()));
    let mut made = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|cx| AgentSession::start("k".into(), agent, project, None, window, cx));
        made = Some(session.clone());
        Root { _session: session }
    });
    cx.run_until_parked();
    (made.unwrap(), fake, cx)
}

/// Keeps the session alive in the window.
struct Root {
    _session: Entity<AgentSession>,
}

impl gpui_kit::Render for Root {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}

/// A message works, a question needs the reader, an answer sends the choice it offers, and the turn
/// ends finished while the reader looks elsewhere.
#[gpui_kit::test]
fn a_turn_asks_is_answered_and_ends_finished(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![Event::Permission(ask())]], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("write a.txt".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::NeedsYou(beui::session_status::Need::Approval));
    assert_eq!(cx.update(|_, cx| session.read(cx).title.to_string()), "write a.txt", "the first message names it");
    cx.update(|_, cx| session.update(cx, |s, cx| s.answer(&RequestId::new("r1"), ChoiceKind::AllowAlways, cx)));
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::Answer { choice, .. } if choice.0 == "always")));
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Working);
    fake.turns.lock().unwrap().push(vec![ended()]);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("go on".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Finished, "unseen: amber");
    cx.update(|_, cx| session.update(cx, |s, cx| s.set_seen(true, cx)));
    assert_eq!(cx.update(|_, cx| session.read(cx).status.clone()), SessionStatus::Idle, "opening clears it");
}

/// A session whose agent could not start tries again when the reader writes, and the message goes.
#[gpui_kit::test]
fn a_failed_start_is_retried_by_the_next_message(cx: &mut TestAppContext) {
    let (session, fake, cx) = start(cx, vec![vec![ended()]], true);
    assert!(cx.update(|_, cx| session.read(cx).problem.clone()).is_some_and(|p| p.contains("not installed")));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| session.read(cx).running()), "it started this time");
    assert!(fake.received.lock().unwrap().iter().any(|c| matches!(c, Command::Send { text, .. } if text == "hello")), "and the message went");
}

/// Streaming text joins into one row, and only the rows that changed are measured again.
#[gpui_kit::test]
fn a_stream_folds_into_one_row(cx: &mut TestAppContext) {
    let block = lathe_agents::session::BlockId(1);
    let turn = (0..50).map(|i| Event::Text { block, delta: format!("word{i} ") }).chain([ended()]).collect();
    let (session, _, cx) = start(cx, vec![turn], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("talk".into(), cx)));
    cx.run_until_parked();
    let (items, rows) = cx.update(|_, cx| {
        let s = session.read(cx);
        (s.conversation.items().len(), s.list.item_count())
    });
    assert_eq!(items, 2, "the message and one text");
    assert_eq!(rows, items, "the list holds a row for each");
}
