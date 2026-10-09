//! The gateway as an agent meets it: a real HTTP client against the real server on 127.0.0.1.
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, RwLock},
    time::Duration,
};

use atelier_capabilities::{
    Actor, Registry,
    tasks::{MemoryTasks, NewTask, TasksProvider},
};
use serde_json::{Value, json};

use crate::{Gateway, SessionAccess, TasksTools, ToolSet};

mod access;
mod protocol;
mod tasks;

const VERSION: &str = "2025-11-25";

/// A gateway with the tasks tools over one memory provider, and a session token for an agent working for Alex.
struct Fixture {
    gateway: Gateway,
    memory: Arc<MemoryTasks>,
    access: SessionAccess,
    http: ureq::Agent,
}

fn agent() -> Actor {
    Actor::agent("claude-1", "Claude Code", "alex")
}

fn fixture() -> Fixture {
    let memory = Arc::new(MemoryTasks::new("demo"));
    let mut registry = Registry::new();
    registry.add_tasks(memory.clone());
    with_registry(registry, memory)
}

fn with_registry(registry: Registry, memory: Arc<MemoryTasks>) -> Fixture {
    let tools: Arc<dyn ToolSet> = Arc::new(TasksTools::new(Arc::new(RwLock::new(registry))));
    let gateway = Gateway::start(vec![tools]).expect("the gateway starts");
    let access = gateway.session(agent()).expect("a session token");
    let http = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    Fixture {
        gateway,
        memory,
        access,
        http,
    }
}

/// A person made this task; the memory provider keeps it.
fn seed(f: &Fixture, title: &str) -> atelier_capabilities::tasks::Task {
    f.memory
        .create(&NewTask::titled(title), &Actor::person("alex", "Alex"))
        .expect("a seeded task")
}

impl Fixture {
    /// One POST. Returns the HTTP status and the body as JSON (`Null` when the body is empty).
    fn post(&self, token: Option<&str>, headers: &[(&str, &str)], body: &Value) -> (u16, Value) {
        let mut request = self
            .http
            .post(&self.access.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let mut response = request.send(body.to_string()).expect("the server answers");
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().expect("a body");
        let json = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).expect("a JSON body")
        };
        (status, json)
    }

    /// A request that carries this fixture's own token and the protocol version.
    fn rpc(&self, method: &str, params: Value) -> Value {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let (status, answer) = self.post(
            Some(&self.access.token),
            &[("MCP-Protocol-Version", VERSION)],
            &body,
        );
        assert_eq!(status, 200, "{answer}");
        answer
    }

    /// The `result` of a `tools/call`.
    fn call(&self, tool: &str, arguments: Value) -> Value {
        let answer = self.rpc(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        assert!(answer.get("error").is_none(), "{answer}");
        answer["result"].clone()
    }

    /// The text of the first content block of a result.
    fn text(result: &Value) -> String {
        result["content"][0]["text"]
            .as_str()
            .expect("a text block")
            .to_string()
    }
}

/// Raw bytes to the server, for what an HTTP client will not send (a Host that lies).
fn raw(port_of: &str, request: &str) -> String {
    let address = port_of
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
        .expect("an http url");
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    stream.write_all(request.as_bytes()).expect("write");
    let mut text = String::new();
    // The server closes the connection after the answer.
    stream.read_to_string(&mut text).ok();
    text
}
