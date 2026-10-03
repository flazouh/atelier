//! A session that continues another one, on another agent or provider. The other session's history becomes a brief
//! (`atelier_agents::handoff`) that goes with the first message; the message shows as the reader typed it. The whole
//! transcript is kept in the project's data folder, and the brief names it, so the agent can look things up.

use std::sync::Arc;

use atelier_agents::{
    handoff::{self, Origin},
    session::{Backend, Command, SessionId},
};
use atelier_project::Project;
use gpui_kit::{AppContext, Context, SharedString, Task};

use super::AgentSession;

const TRANSCRIPT_FOLDER: &str = "handoffs";
const ASK_NOW: &str = "## What the user asks now";
const WORKING_STATE: [&[&str]; 2] = [&["status", "--short", "--branch"], &["diff", "--stat"]];

/// The session a new one continues.
#[derive(Clone)]
pub struct Source {
    pub backend: Arc<dyn Backend>,
    /// The agent's name, as the brief tells it.
    pub agent: String,
    pub id: SessionId,
    pub title: SharedString,
}

pub(super) struct Handoff {
    source: Source,
    /// `None` while the other session is read; why not, when it could not be.
    brief: Option<Result<String, String>>,
    /// The first message, waiting for the brief.
    waiting: Option<Command>,
    _reading: Task<()>,
}

impl AgentSession {
    /// Continues `source`: its brief goes with the first message. It runs on `provider` when that is given and the
    /// agent runs on providers; the agent starts again on it.
    pub fn continue_from(&mut self, source: Source, provider: Option<crate::providers::Choice>, cx: &mut Context<Self>) {
        let changes_provider = self.provider.is_some() && provider.is_some() && self.provider != provider;
        if changes_provider {
            self.provider = provider;
        }
        let (project, reading_source) = (self.project.clone(), source.clone());
        let reading = cx.background_spawn(async move { read_brief(&reading_source, project.as_ref()) });
        let _reading = cx.spawn(async move |this, cx| {
            let brief = reading.await;
            _ = this.update(cx, |s, cx| s.brief_read(brief, cx));
        });
        self.handoff = Some(Handoff { source, brief: None, waiting: None, _reading });
        if changes_provider || self.continues_natively() {
            self.restart(cx);
        }
        cx.notify();
    }

    /// It resumes a fork of the session it continues, so it needs no brief: the same agent, one that forks, on one
    /// of its accounts. OpenRouter takes the brief, as thinking signed by one backend may fail on another.
    pub fn continues_natively(&self) -> bool {
        self.native_fork().is_some()
    }

    /// The session to fork when it starts, while its first message has not gone.
    pub(super) fn native_fork(&self) -> Option<SessionId> {
        let handoff = self.handoff.as_ref()?;
        let same_agent = handoff.source.backend.name() == self.agent.backend.name();
        let on_an_account = matches!(self.provider, Some(crate::providers::Choice::Account(_)));
        (same_agent && on_an_account && self.agent.backend.capabilities().forks).then(|| handoff.source.id.clone())
    }

    /// The session this one continues, while its first message has not gone.
    pub fn continues(&self) -> Option<&Source> {
        self.handoff.as_ref().map(|h| &h.source)
    }

    /// The message to send now: `command` with the brief in front, or `None` while the brief is read, when it goes
    /// once read. Every message after the first goes as it is.
    pub(super) fn with_brief(&mut self, command: Command, cx: &mut Context<Self>) -> Option<Command> {
        if self.continues_natively() {
            self.handoff = None;
            return Some(command);
        }
        let Some(handoff) = &mut self.handoff else { return Some(command) };
        match handoff.brief.take() {
            None => {
                handoff.waiting = Some(command);
                None
            }
            Some(brief) => {
                self.handoff = None;
                Some(self.joined(brief, command, cx))
            }
        }
    }

    fn brief_read(&mut self, brief: Result<String, String>, cx: &mut Context<Self>) {
        let Some(handoff) = &mut self.handoff else { return };
        match handoff.waiting.take() {
            Some(command) => {
                self.handoff = None;
                let command = self.joined(brief, command, cx);
                self.dispatch(command, cx);
            }
            None => handoff.brief = Some(brief),
        }
        cx.notify();
    }

    /// `command` with `brief` in front; a brief that could not be read leaves the message as it is and says why.
    fn joined(&mut self, brief: Result<String, String>, command: Command, cx: &mut Context<Self>) -> Command {
        match (brief, command) {
            (Ok(brief), Command::Send { text, attachments }) => Command::Send { text: format!("{brief}\n\n{ASK_NOW}\n\n{text}"), attachments },
            (Err(why), command) => {
                self.problem = Some(format!("The session it continues could not be read, so the message went alone: {why}").into());
                cx.notify();
                command
            }
            (_, command) => command,
        }
    }
}

/// Reads `source`'s history, keeps its transcript in the data folder, and makes the brief. Blocks: keep it off the
/// UI thread.
fn read_brief(source: &Source, project: &dyn Project) -> Result<String, String> {
    let events = source.backend.history(project, &source.id).map_err(|e| e.to_string())?;
    let mut origin = Origin { agent: source.agent.clone(), title: source.title.to_string(), transcript: None, working_state: working_state(project) };
    let file = format!("{TRANSCRIPT_FOLDER}/{}.md", source.id.0);
    if project.data_write(&file, handoff::transcript(&events, &origin).as_bytes()).is_ok() {
        origin.transcript = project.data_path().map(|folder| folder.join(&file).display().to_string());
    }
    Ok(handoff::brief(&events, &origin, handoff::BUDGET))
}

/// The branch and changed files, as git tells them; `None` outside a repository.
fn working_state(project: &dyn Project) -> Option<String> {
    let told: Vec<String> = WORKING_STATE
        .iter()
        .filter_map(|args| project.git(args).ok().filter(|out| out.code == Some(0)).map(|out| out.stdout.trim_end().to_string()))
        .filter(|text| !text.is_empty())
        .collect();
    (!told.is_empty()).then(|| told.join("\n\n"))
}
