//! The Claude Code backend: `claude` run as a child of the project (so a remote project runs it on
//! its host), spoken to in stream-json. `wire` reads its lines, `map` turns them into atelier's events,
//! `control` writes the lines it needs, `launch` builds its command line, `store` reads its past
//! sessions. Only `session` touches a process; the rest is pure and tested on captured runs.
//! The protocol is written down in `docs/agents.md`.
pub mod accounts;
mod control;
mod launch;
mod map;
mod session;
mod store;
mod tools;
mod wire;

use std::sync::Arc;

use atelier_project::Project;

pub use map::{ClaudeLineMapper, LineMapper};
pub use store::history;

use crate::{
    session::{
        Account, Backend, Capabilities, EventSink, ModelChoice, OpenRequest, PermissionMode, Session, SessionError,
        SessionId, SessionSummary,
    },
    subprocess,
};

pub struct ClaudeCode {
    program: String,
}

impl ClaudeCode {
    /// Runs the `claude` the host's `PATH` finds.
    pub fn new() -> Self {
        Self::with_program("claude")
    }

    pub fn with_program(program: impl Into<String>) -> Self {
        Self { program: program.into() }
    }
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for ClaudeCode {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn capabilities(&self) -> Capabilities {
        let model = |id: &str, label: &str| ModelChoice { id: id.into(), label: label.into() };
        Capabilities {
            resume: true,
            interrupt: true,
            models: vec![model("opus", "Opus"), model("sonnet", "Sonnet"), model("haiku", "Haiku")],
            permission_modes: vec![
                PermissionMode::Ask,
                PermissionMode::AcceptEdits,
                PermissionMode::Plan,
                PermissionMode::Auto,
                PermissionMode::Bypass,
            ],
            thinking: true,
            subagents: true,
            todos: true,
            providers: true,
            forks: true,
        }
    }

    fn open(
        &self,
        project: Arc<dyn Project>,
        request: OpenRequest,
        sink: EventSink,
    ) -> Result<Box<dyn Session>, SessionError> {
        let command = launch::command(&self.program, &request);
        let process = subprocess::start(project.as_ref(), &command)?;
        Ok(Box::new(session::ClaudeSession::run(process, sink)))
    }

    fn accounts(&self, project: &dyn Project) -> Result<Vec<Account>, SessionError> {
        accounts::accounts(project, &self.program)
    }

    fn session_account(&self, project: &dyn Project, session: &SessionId) -> Result<Option<String>, SessionError> {
        store::holder(project, session)
    }

    fn sign_in(&self, account: &str) -> Option<atelier_project::Command> {
        Some(accounts::sign_in_command(&self.program, account))
    }

    fn sessions(&self, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
        store::list(project)
    }

    fn history(
        &self,
        project: &dyn Project,
        session: &SessionId,
    ) -> Result<Vec<crate::session::Event>, SessionError> {
        store::read_history(project, session)
    }

    fn draft(&self, project: &dyn Project, prompt: &str, model: Option<&str>) -> Result<String, SessionError> {
        use std::io::{Read, Write};
        let mut process = subprocess::start(project, &launch::draft_command(&self.program, model))?;
        let written = process.stdin.write_all(prompt.as_bytes());
        drop(process.stdin);
        let mut text = String::new();
        let read = process.stdout.read_to_string(&mut text);
        let code = process.control.wait().map_err(|e| SessionError::Start(e.to_string()))?;
        if code != Some(0) || written.is_err() || read.is_err() {
            let stderr = process.control.stderr();
            let why = stderr.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string);
            return Err(SessionError::Start(why.unwrap_or_else(|| format!("the draft ended with {code:?}"))));
        }
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests;
