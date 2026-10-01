use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::{
    AcpAgent,
    protocol::{Goal, Protocol, Step},
};
use crate::session::{Command, Event, ModelChoice, OpenRequest, PermissionMode};

mod map;
mod process;
mod protocol;
mod rpc;
mod time;

/// An agent with two modes and two models, as a test names it.
fn agent() -> AcpAgent {
    AcpAgent {
        name: "test-acp",
        program: "acp-agent".into(),
        args: vec!["acp".into()],
        modes: vec![(PermissionMode::Ask, "agent".into()), (PermissionMode::Plan, "plan".into())],
        models: vec![ModelChoice { id: "auto".into(), label: "Auto".into() }, ModelChoice { id: "fast".into(), label: "Fast".into() }],
        thinking: true,
        todos: true,
    }
}

/// A protocol driven by hand: what atelier wrote and the events it gave, each line one millisecond after the last.
struct Run {
    protocol: Protocol,
    written: Vec<Value>,
    events: Vec<Event>,
    start: Instant,
    ticks: u64,
    done: bool,
}

impl Run {
    fn new(goal: Goal) -> Self {
        let (protocol, first) = Protocol::new(Arc::new(agent()), "/work/project".into(), goal);
        let mut run = Self { protocol, written: Vec::new(), events: Vec::new(), start: Instant::now(), ticks: 0, done: false };
        run.take(Step { lines: first, ..Step::default() });
        run
    }

    fn open() -> Self {
        Self::new(Goal::Open(OpenRequest::default()))
    }

    /// A session the agent has started, `s1`, in mode `agent` on model `auto`, set through a config option.
    fn ready() -> Self {
        let mut run = Self::open();
        run.initialized(json!({}));
        run.agent(respond(1, json!({
            "sessionId": "s1",
            "modes": { "currentModeId": "agent", "availableModes": [{ "id": "agent", "name": "Agent" }, { "id": "plan", "name": "Plan" }] },
            "configOptions": [{ "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "auto", "options": [] }],
        })));
        run
    }

    /// The agent answers `initialize` (always atelier's request 0) with `capabilities`.
    fn initialized(&mut self, capabilities: Value) -> &mut Self {
        self.agent(respond(0, json!({ "protocolVersion": 1, "agentCapabilities": capabilities, "authMethods": [{ "id": "cursor_login", "name": "Cursor Login" }] })))
    }

    fn agent(&mut self, message: Value) -> &mut Self {
        self.ticks += 1;
        let step = self.protocol.line(&message.to_string(), self.start + Duration::from_millis(self.ticks));
        self.take(step);
        self
    }

    fn command(&mut self, command: Command) -> &mut Self {
        let step = self.protocol.command(command).expect("the command is taken");
        self.take(step);
        self
    }

    fn take(&mut self, step: Step) {
        self.written.extend(step.lines.iter().map(|line| serde_json::from_str::<Value>(line).expect("atelier writes JSON")));
        self.events.extend(step.events);
        self.done |= step.done;
    }

    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn last(&self) -> &Value {
        self.written.last().expect("atelier wrote something")
    }

    /// The requests atelier wrote for `method`.
    fn sent(&self, method: &str) -> Vec<&Value> {
        self.written.iter().filter(|line| line["method"] == method).collect()
    }
}

fn respond(id: u64, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn fail(id: u64, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn update(update: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "sessionId": "s1", "update": update } })
}

fn chunk(kind: &str, text: &str) -> Value {
    update(json!({ "sessionUpdate": kind, "content": { "type": "text", "text": text } }))
}
