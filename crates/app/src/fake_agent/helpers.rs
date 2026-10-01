use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext};
use atelier_agents::session::{Event, TurnEnd, TurnOutcome};
use atelier_project::Project;

use crate::agent_session::AgentSession;
use super::structs::{Fake, FakeBackend, Root};

pub fn ended() -> Event {
    Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Completed, summary: None })
}

pub fn start(cx: &mut TestAppContext, turns: Vec<Vec<Event>>, fail_first: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    start_in(cx, crate::test_dirs::path(), turns, fail_first)
}

/// The same, on a project at `dir`.
pub fn start_in(cx: &mut TestAppContext, dir: PathBuf, turns: Vec<Vec<Event>>, fail_first: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    start_with(cx, dir, turns, fail_first, false)
}

/// The same, with the session's view drawn in the window.
pub fn start_shown_in(cx: &mut TestAppContext, dir: PathBuf, turns: Vec<Vec<Event>>) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    start_with(cx, dir, turns, false, true)
}

fn start_with(cx: &mut TestAppContext, dir: PathBuf, turns: Vec<Vec<Event>>, fail_first: bool, shown: bool) -> (Entity<AgentSession>, Arc<Fake>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Dark, cx);
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
        Root { _session: session, shown }
    });
    cx.run_until_parked();
    (made.unwrap(), fake, cx)
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
