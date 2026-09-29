//! A session on a scripted server, with the events it makes collected.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lathe_project::LocalProject;

use super::server::{FakeServer, Step};
use crate::{
    own::{Anthropic, OwnAgent, OwnOptions, RetryPolicy, Secret},
    session::{Backend, Command, Event, OpenRequest, PermissionMode, Session, SessionId, TurnEnd},
};

pub const KEY: &str = "sk-ant-test-0123456789-DO-NOT-LEAK";

pub fn options() -> OwnOptions {
    OwnOptions { retry: RetryPolicy { max_retries: 2, base: Duration::from_millis(10), max_delay: Duration::from_millis(40) }, ..OwnOptions::default() }
}

pub struct Rig {
    pub dir: tempfile::TempDir,
    /// The project's data folder, where the record goes.
    pub data: tempfile::TempDir,
    pub project: Arc<dyn lathe_project::Project>,
    pub agent: OwnAgent,
    pub server: FakeServer,
    pub events: Arc<Mutex<Vec<Event>>>,
    pub session: Option<Box<dyn Session>>,
}

pub fn rig(steps: Vec<Step>, mode: PermissionMode) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project: Arc<dyn lathe_project::Project> = Arc::new(crate::testing::Locked(Arc::new(LocalProject::open(dir.path()).unwrap().with_data_dir(data.path()))));
    let server = FakeServer::start(steps);
    let model = Anthropic::new(Secret::new(KEY)).with_base(server.url.clone());
    let agent = OwnAgent::new(Arc::new(model), options());
    let mut rig = Rig { dir, data, project, agent, server, events: Arc::default(), session: None };
    rig.open(OpenRequest { mode: Some(mode), ..OpenRequest::default() });
    rig
}

impl Rig {
    pub fn open(&mut self, request: OpenRequest) {
        let events = self.events.clone();
        let sink: crate::session::EventSink = Arc::new(move |e| events.lock().unwrap().push(e));
        self.session = Some(self.agent.open(self.project.clone(), request, sink).unwrap());
    }

    pub fn send(&self, text: &str) {
        self.session.as_ref().unwrap().send(Command::send(text)).unwrap();
    }

    pub fn command(&self, command: Command) {
        self.session.as_ref().unwrap().send(command).unwrap();
    }

    pub fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }

    /// Waits until `pred` holds for the events so far. Panics with the events after ten seconds.
    pub fn wait_for(&self, what: &str, pred: impl Fn(&[Event]) -> bool) -> Vec<Event> {
        let start = Instant::now();
        loop {
            let events = self.events();
            if pred(&events) {
                return events;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "waited for {what}; the events were {events:#?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits until `n` turns have ended.
    pub fn turns(&self, n: usize) -> Vec<Event> {
        self.wait_for(&format!("{n} turn(s) to end"), |e| e.iter().filter(|e| matches!(e, Event::TurnEnded(_))).count() >= n)
    }

    pub fn session_id(&self) -> SessionId {
        self.events().iter().find_map(|e| if let Event::Started(s) = e { Some(s.session.clone()) } else { None }).expect("a Started event")
    }
}

pub fn turn_ends(events: &[Event]) -> Vec<TurnEnd> {
    events.iter().filter_map(|e| if let Event::TurnEnded(t) = e { Some(t.clone()) } else { None }).collect()
}

pub fn text_of(events: &[Event]) -> String {
    events.iter().filter_map(|e| if let Event::Text { delta, .. } = e { Some(delta.as_str()) } else { None }).collect()
}
