//! A backend with no process, no wire and no thread: it answers each message with scripted events, in
//! the caller's thread. It proves the trait fits a backend that lives inside lathe, and gives the
//! model's tests a session that behaves the same every run.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use lathe_project::Project;

use super::{
    Backend, Capabilities, Command, EndReason, Event, EventSink, OpenRequest, Session, SessionError, SessionId,
    Started, TurnEnd, TurnOutcome,
};

pub struct FakeBackend {
    turns: Vec<Vec<Event>>,
    capabilities: Capabilities,
    received: Arc<Mutex<Vec<Command>>>,
}

impl FakeBackend {
    /// One batch of events per user message, in order.
    pub fn new(turns: Vec<Vec<Event>>) -> Self {
        Self { turns, capabilities: Capabilities::default(), received: Arc::default() }
    }

    /// What the fake says it supports; nothing until it says.
    pub fn offering(mut self, capabilities: Capabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// The commands its sessions received.
    pub fn received(&self) -> Vec<Command> {
        self.received.lock().unwrap().clone()
    }
}

impl Backend for FakeBackend {
    fn name(&self) -> &str {
        "fake"
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn open(
        &self,
        _project: Arc<dyn Project>,
        request: OpenRequest,
        sink: EventSink,
    ) -> Result<Box<dyn Session>, SessionError> {
        let id = request.resume.unwrap_or_else(|| SessionId::new("fake-1"));
        sink(Event::Started(Started { session: id, model: request.model, mode: request.mode , commands: Vec::new() }));
        Ok(Box::new(FakeSession {
            turns: Mutex::new(self.turns.clone().into()),
            sink,
            received: self.received.clone(),
        }))
    }
}

struct FakeSession {
    turns: Mutex<VecDeque<Vec<Event>>>,
    sink: EventSink,
    received: Arc<Mutex<Vec<Command>>>,
}

impl Session for FakeSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        self.received.lock().unwrap().push(command.clone());
        match command {
            Command::Send { .. } => {
                let turn = self.turns.lock().unwrap().pop_front().ok_or(SessionError::Closed)?;
                turn.into_iter().for_each(|event| (self.sink)(event));
            }
            Command::Interrupt => {
                (self.sink)(Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Interrupted, summary: None }));
            }
            Command::Answer { .. } | Command::SetModel { .. } | Command::SetPermissionMode { .. } => {}
        }
        Ok(())
    }
}

impl Drop for FakeSession {
    fn drop(&mut self) {
        (self.sink)(Event::Ended(EndReason::Closed));
    }
}
