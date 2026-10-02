use std::sync::{Arc, Mutex};

use gpui_kit::Entity;
use atelier_agents::session::{
    Backend, Capabilities, Command, Event, EventSink, OpenRequest, Session, SessionError,
    SessionId, Started,
};
use atelier_project::Project;

use crate::agent_session::AgentSession;

/// A backend in the test's thread: each message plays the next scripted turn into the sink, and every
/// command is kept. `fail_first` makes the first open fail as a missing program does.
pub struct Fake {
    pub turns: Mutex<Vec<Vec<Event>>>,
    pub received: Arc<Mutex<Vec<Command>>>,
    pub(super) fail_first: Mutex<bool>,
    /// Runs before each message's turn plays, as the agent's own work on the files would.
    pub work: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

struct FakeSession {
    pub(super) backend: Arc<Fake>,
    pub(super) sink: EventSink,
}

impl Session for FakeSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        let turn = matches!(command, Command::Send { .. });
        self.backend.received.lock().unwrap().push(command);
        if turn {
            let work = { let mut w = self.backend.work.lock().unwrap(); (!w.is_empty()).then(|| w.remove(0)) };
            if let Some(work) = work {
                work();
            }
            let next = { let mut t = self.backend.turns.lock().unwrap(); if t.is_empty() { Vec::new() } else { t.remove(0) } };
            next.into_iter().for_each(|e| (self.sink)(e));
        }
        Ok(())
    }
}

pub(super) struct FakeBackend(pub(super) Arc<Fake>);

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
        sink(Event::Started(Started { session: id, model: None, mode: None, commands: Vec::new() }));
        Ok(Box::new(FakeSession { backend: self.0.clone(), sink }))
    }

    /// A branch name for a branch prompt, a message for any other.
    fn draft(&self, _: &dyn Project, prompt: &str, _: Option<&str>) -> Result<String, SessionError> {
        Ok(if prompt.contains("Name a git branch") { "fix/keep-two".into() } else { "Keep TWO\n\nFrom the review.".into() })
    }
}

/// Keeps the session alive in the window.
pub(super) struct Root {
    pub(super) _session: Entity<AgentSession>,
}

impl gpui_kit::Render for Root {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}
