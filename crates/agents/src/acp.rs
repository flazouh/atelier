//! The ACP backend: any agent that speaks the Agent Client Protocol (JSON-RPC 2.0 over its stdin and
//! stdout), run as a child of the project so a remote project runs it on its host. One backend serves
//! every such agent; an [`AcpAgent`] says how to start one and how atelier's modes and models map to its own.
//! `rpc` frames messages, `wire` reads the agent's payloads, `client` writes atelier's, `map` turns session
//! updates into atelier's events, and `protocol` runs the conversation (handshake, sign-in, turns,
//! questions) as pure state. Only `session` and `store` touch a process. The protocol, and what each
//! agent was checked against, is written down in `docs/agents.md`.
mod client;
mod map;
mod protocol;
mod rpc;
mod session;
mod store;
mod time;
mod wire;

use std::sync::Arc;

use atelier_project::{Command, Project};

use crate::{
    session::{
        Backend, Capabilities, Event, EventSink, ModelChoice, OpenRequest, PermissionMode, Session, SessionError,
        SessionId, SessionSummary,
    },
    subprocess,
};

/// How atelier starts one ACP agent, and the names it gives atelier's modes and models. Everything else comes
/// over the protocol.
#[derive(Clone, Debug)]
pub struct AcpAgent {
    /// The backend's stable name, for settings and a session's record, such as `cursor`.
    pub name: &'static str,
    /// A path on the host, or a name its `PATH` finds.
    pub program: String,
    pub args: Vec<String>,
    /// Each mode atelier offers, with the agent's id for it, in the order a picker shows them.
    pub modes: Vec<(PermissionMode, String)>,
    /// The models a session can switch to, by the id the agent takes.
    pub models: Vec<ModelChoice>,
    /// Whether the agent streams its thinking.
    pub thinking: bool,
    /// Whether the agent keeps a plan, which atelier shows as its todo list.
    pub todos: bool,
}

impl AcpAgent {
    /// The same agent, started from `program` in place of the one the host's `PATH` finds.
    pub fn with_program(self, program: impl Into<String>) -> Self {
        Self { program: program.into(), ..self }
    }

    fn command(&self) -> Command {
        Command::new(&self.program).args(self.args.iter().cloned())
    }

    /// The agent's id for atelier's `mode`, when it has one.
    fn mode_id(&self, mode: PermissionMode) -> Option<&str> {
        self.modes.iter().find(|(m, _)| *m == mode).map(|(_, id)| id.as_str())
    }

    /// atelier's mode for the agent's mode `id`, when atelier has one.
    fn mode_for(&self, id: &str) -> Option<PermissionMode> {
        self.modes.iter().find(|(_, m)| m == id).map(|(mode, _)| *mode)
    }
}

pub struct Acp {
    agent: Arc<AcpAgent>,
}

impl Acp {
    pub fn new(agent: AcpAgent) -> Self {
        Self { agent: Arc::new(agent) }
    }
}

impl Backend for Acp {
    fn name(&self) -> &str {
        self.agent.name
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            resume: true,
            interrupt: true,
            models: self.agent.models.clone(),
            permission_modes: self.agent.modes.iter().map(|(mode, _)| *mode).collect(),
            thinking: self.agent.thinking,
            subagents: false,
            todos: self.agent.todos,
        }
    }

    fn open(&self, project: Arc<dyn Project>, request: OpenRequest, sink: EventSink) -> Result<Box<dyn Session>, SessionError> {
        let process = subprocess::start(project.as_ref(), &self.agent.command())?;
        let cwd = project.root().display().to_string();
        let (protocol, first) = protocol::Protocol::new(self.agent.clone(), cwd, protocol::Goal::Open(request));
        Ok(Box::new(session::AcpSession::run(process, protocol, first, sink)))
    }

    fn sessions(&self, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
        store::list(self.agent.clone(), project)
    }

    fn history(&self, project: &dyn Project, session: &SessionId) -> Result<Vec<Event>, SessionError> {
        store::history(self.agent.clone(), project, session)
    }
}

#[cfg(test)]
mod tests;
