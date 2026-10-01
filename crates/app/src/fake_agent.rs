//! A backend for tests, in the test's thread: each message plays the next scripted turn into the sink,
//! after the scripted work on the files, and every command is kept.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext};
use atelier_agents::session::{
    Backend, Capabilities, Command, Event, EventSink, OpenRequest, Session, SessionError, SessionId, Started, TurnEnd, TurnOutcome,
};
use atelier_project::Project;

use crate::agent_session::AgentSession;

/// A backend in the test's thread: each message plays the next scripted turn into the sink, and every
/// command is kept. `fail_first` makes the first open fail as a missing program does.
pub struct Fake {
    pub turns: Mutex<Vec<Vec<Event>>>,
    pub received: Arc<Mutex<Vec<Command>>>,
    fail_first: Mutex<bool>,
    /// Runs before each message's turn plays, as the agent's own work on the files would.
    pub work: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
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
        sink(Event::Started(Started { session: id, model: None, mode: None, commands: Vec::new() }));
        Ok(Box::new(FakeSession { backend: self.0.clone(), sink }))
    }

    /// A branch name for a branch prompt, a message for any other.
    fn draft(&self, _: &dyn Project, prompt: &str, _: Option<&str>) -> Result<String, SessionError> {
        Ok(if prompt.contains("Name a git branch") { "fix/keep-two".into() } else { "Keep TWO\n\nFrom the review.".into() })
    }
}

pub fn ended() -> Event {
    Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Completed, summary: None })
}

pub fn start(cx: &mut TestAppContext, turns: Vec<Vec<Event>>, fail_first: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    start_in(cx, crate::test_dirs::path(), turns, fail_first)
}

/// The same, on a project at `dir`.
pub fn start_in(cx: &mut TestAppContext, dir: PathBuf, turns: Vec<Vec<Event>>, fail_first: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let fake = Arc::new(Fake { turns: Mutex::new(turns), received: Arc::default(), fail_first: Mutex::new(fail_first), work: Mutex::default() });
    // Its data folder is the test's own, never this machine's.
    let data = crate::test_dirs::path();
    let project: Arc<dyn Project> = Arc::new(atelier_project::LocalProject::open(&dir).unwrap().with_data_dir(&data));
    let mut agent = atelier_agents::registry::agents().remove(0);
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


/// A git repository with `files` committed, for a turn's review to diff against.
pub fn git_project(files: &[(&str, &str)]) -> PathBuf {
    git_project_in(crate::test_dirs::path(), files)
}

/// The same, in `dir`, which may hold files already: all of them are committed.
pub fn git_project_in(dir: PathBuf, files: &[(&str, &str)]) -> PathBuf {
    for (path, text) in files {
        std::fs::write(dir.join(path), text).unwrap();
    }
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"]).args(args).current_dir(&dir).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "start"]);
    dir
}

/// An agent named `name` on a fake backend, for a test that starts a session itself.
pub fn fake_agent(name: &'static str) -> atelier_agents::registry::Agent {
    let fake = Arc::new(Fake { turns: Mutex::default(), received: Arc::default(), fail_first: Mutex::new(false), work: Mutex::default() });
    let mut agent = atelier_agents::registry::agents().remove(0);
    agent.backend = Arc::new(FakeBackend(fake));
    agent.name = name;
    agent
}
/// An agent named `name` on a fake backend that plays `turns`, one for each message, and that backend.
pub fn scripted_agent(name: &'static str, turns: Vec<Vec<Event>>) -> (atelier_agents::registry::Agent, Arc<Fake>) {
    let fake = Arc::new(Fake { turns: Mutex::new(turns), received: Arc::default(), fail_first: Mutex::new(false), work: Mutex::default() });
    let mut agent = atelier_agents::registry::agents().remove(0);
    agent.backend = Arc::new(FakeBackend(fake.clone()));
    agent.name = name;
    (agent, fake)
}
