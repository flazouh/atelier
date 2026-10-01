//! atelier's own agent: an agent loop that runs inside atelier and calls a model API directly. No child
//! process and no wire format of ours; it is a [`Backend`] like the others, and the UI cannot tell.
//!
//! - [`Model`] is the one seam to a model API: a streaming chat call with tools. [`Anthropic`] is the
//!   Messages API; [`OpenAiCompatible`] is Chat Completions, which covers OpenAI, OpenRouter and the many
//!   servers that copy it.
//! - The tools (`read`, `list`, `search`, `edit`, `write`, `shell`) touch the project only through
//!   `Project`, so the agent works on a local folder and on an SSH host with no other code.
//! - Permissions are atelier's own ([`permission`]). The record of a session is kept in the project
//!   ([`store`]), so a session resumes with no service behind it.
//!
//! Keys come from the caller: [`OwnAgent::from_env`] reads the environment, and a settings screen can
//! hand a [`Secret`] to the constructors. A key is never logged, stored by this module, or put in an event.
use std::{path::Path, sync::Arc, time::Duration};

use atelier_project::Project;

pub mod anthropic;
pub mod context;
pub mod http;
pub mod message;
pub mod openai;
pub mod permission;
pub mod sse;
pub mod store;
pub mod tools;

mod runner;

pub use anthropic::Anthropic;
pub use context::Budget;
pub use message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, Reply, Role, Secret, StopReason, Thinking, TokenUsage, ToolDef};
pub use openai::OpenAiCompatible;

use crate::session::{
    Backend, Capabilities, EventSink, ModelChoice, OpenRequest, PermissionMode, Session, SessionError, SessionId, SessionSummary,
};

/// How a failed call is tried again: `max_retries` more times, after `base`, then twice as long, up to
/// `max_delay`. A rate limit that names its own wait is obeyed, up to `max_delay`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_retries: 3, base: Duration::from_secs(1), max_delay: Duration::from_secs(30) }
    }
}

impl RetryPolicy {
    pub fn delay(&self, attempt: u32) -> Duration {
        (self.base * 2u32.saturating_pow(attempt)).min(self.max_delay)
    }
}

#[derive(Clone, Debug)]
pub struct OwnOptions {
    /// The models a session can switch to.
    pub models: Vec<ModelChoice>,
    pub default_model: String,
    /// The most tokens of one reply.
    pub max_tokens: u32,
    pub thinking: Thinking,
    /// Added to the built-in system prompt: the project's own instructions.
    pub system_extra: String,
    pub context: Budget,
    pub retry: RetryPolicy,
    /// Tools allowed without asking, in every mode but Plan: the reader's "always allow" rules.
    pub allow: Vec<String>,
    /// The most model calls in one turn.
    pub max_steps: usize,
}

impl Default for OwnOptions {
    fn default() -> Self {
        Self {
            models: anthropic_models(),
            default_model: "claude-opus-5-5".into(),
            max_tokens: 16_000,
            thinking: Thinking::Auto,
            system_extra: String::new(),
            context: Budget::default(),
            retry: RetryPolicy::default(),
            allow: Vec::new(),
            max_steps: 60,
        }
    }
}

fn choice(id: &str, label: &str) -> ModelChoice {
    ModelChoice { id: id.into(), label: label.into() }
}

/// The Claude models to offer before the account's own list is known (see [`Anthropic::models`]).
pub fn anthropic_models() -> Vec<ModelChoice> {
    vec![choice("claude-opus-5-5", "Opus 5.5"), choice("claude-fable-5-1", "Fable 5.1"), choice("claude-sonnet-5-5", "Sonnet 5.5"), choice("claude-sonnet-5", "Sonnet 5"), choice("claude-haiku-4-5", "Haiku 4.5")]
}

/// The instructions every session starts with. It holds nothing that changes from call to call (no
/// clock), so the prompt cache keeps it.
pub fn system_prompt(root: &Path, extra: &str) -> String {
    let mut prompt = format!(
        "You are atelier's coding agent. You work in the project \"{}\" (folder: {}). You help the user \
         read, understand and change its code.\n\n\
         Work like this:\n\
         - Look before you change. Use `search` and `list` to find code, and `read` to read it.\n\
         - Change files with `edit` (an exact replacement) for a part of a file and `write` for a whole file. \
           Read a file before you edit it. Keep changes small and in the style of the code around them.\n\
         - Use `shell` to run the project's tests, build and other commands. Its output is cut when long.\n\
         - A tool result that says it failed is information: read it, then fix the cause. Do not repeat a call that failed.\n\
         - If the user does not allow an action, do not try it again. Ask what they want.\n\
         - Say briefly what you did and what you found. Do not paste files back to the user; they can see the changes.\n\
         All paths are relative to the project folder.",
        runner::root_name(root),
        root.display()
    );
    if !extra.trim().is_empty() {
        prompt.push_str("\n\nThe project's instructions:\n");
        prompt.push_str(extra.trim());
    }
    prompt
}

/// A model with no key. Every call says so, in words a person can act on.
struct MissingKey(String);

impl Model for MissingKey {
    fn stream(&self, _: &ModelRequest<'_>, _: &mut dyn FnMut(Delta), _: &Cancel) -> Result<Reply, ModelError> {
        Err(ModelError::Auth(self.0.clone()))
    }
}

pub struct OwnAgent {
    model: Arc<dyn Model>,
    options: Arc<OwnOptions>,
    /// Why the agent cannot start, when it cannot.
    unavailable: Option<String>,
}

fn env_key(name: &str) -> Option<Secret> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).map(Secret::new)
}

impl OwnAgent {
    /// An agent on any [`Model`]. This is the constructor tests and other providers use.
    pub fn new(model: Arc<dyn Model>, options: OwnOptions) -> Self {
        Self { model, options: Arc::new(options), unavailable: None }
    }

    pub fn anthropic(key: Secret) -> Self {
        Self::new(Arc::new(Anthropic::new(key)), OwnOptions::default())
    }

    pub fn openai(key: Secret) -> Self {
        let options = OwnOptions { models: vec![choice("gpt-5", "GPT-5")], default_model: "gpt-5".into(), thinking: Thinking::Off, ..OwnOptions::default() };
        Self::new(Arc::new(OpenAiCompatible::new(Some(key), openai::OPENAI_BASE)), options)
    }

    pub fn openrouter(key: Secret) -> Self {
        let options = OwnOptions {
            models: vec![choice("anthropic/claude-opus-5-5", "Claude Opus 5.5"), choice("openai/gpt-5", "GPT-5")],
            default_model: "anthropic/claude-opus-5-5".into(),
            thinking: Thinking::Off,
            ..OwnOptions::default()
        };
        Self::new(Arc::new(OpenAiCompatible::openrouter(key)), options)
    }

    /// The first key the environment has: `ANTHROPIC_API_KEY`, then `OPENROUTER_API_KEY`, then
    /// `OPENAI_API_KEY`. With none, the agent still exists and says what to set when a session opens.
    pub fn from_env() -> Self {
        if let Some(key) = env_key("ANTHROPIC_API_KEY") {
            Self::anthropic(key)
        } else if let Some(key) = env_key("OPENROUTER_API_KEY") {
            Self::openrouter(key)
        } else if let Some(key) = env_key("OPENAI_API_KEY") {
            Self::openai(key)
        } else {
            let why = "no API key: set ANTHROPIC_API_KEY (or OPENROUTER_API_KEY, or OPENAI_API_KEY)".to_string();
            Self { model: Arc::new(MissingKey(why.clone())), options: Arc::new(OwnOptions::default()), unavailable: Some(why) }
        }
    }

    pub fn options(&self) -> &OwnOptions {
        &self.options
    }
}

impl Backend for OwnAgent {
    fn name(&self) -> &str {
        "atelier"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            resume: true,
            interrupt: true,
            models: self.options.models.clone(),
            permission_modes: vec![PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Bypass],
            thinking: true,
            subagents: false,
            todos: false,
        }
    }

    fn open(&self, project: Arc<dyn Project>, request: OpenRequest, sink: EventSink) -> Result<Box<dyn Session>, SessionError> {
        if let Some(why) = &self.unavailable {
            return Err(SessionError::Start(why.clone()));
        }
        runner::open(self.model.clone(), self.options.clone(), project, request, sink)
    }

    /// One request, with no tools and no record: the model's text as it answers.
    fn draft(&self, _project: &dyn Project, prompt: &str, model: Option<&str>) -> Result<String, SessionError> {
        if let Some(why) = &self.unavailable {
            return Err(SessionError::Start(why.clone()));
        }
        let messages = [message::Message::user(prompt)];
        let request = message::ModelRequest {
            model: model.unwrap_or(&self.options.default_model),
            system: "Answer with the text asked for, and nothing else.",
            tools: &[],
            messages: &messages,
            max_tokens: 2048,
            thinking: Thinking::Off,
        };
        let mut text = String::new();
        self.model
            .stream(&request, &mut |delta| {
                if let message::Delta::Text(more) = delta {
                    text.push_str(&more);
                }
            }, &message::Cancel::default())
            .map_err(|e| SessionError::Start(format!("{e:?}")))?;
        Ok(text.trim().to_string())
    }

    fn sessions(&self, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
        store::list(project)
    }

    fn history(&self, project: &dyn Project, session: &SessionId) -> Result<Vec<crate::session::Event>, SessionError> {
        store::history(project, session)
    }
}

#[cfg(test)]
mod tests;
