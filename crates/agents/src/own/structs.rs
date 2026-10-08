use std::{sync::Arc, time::Duration};

use atelier_project::Project;

pub use super::anthropic::Anthropic;
pub use super::context::Budget;
pub use super::message::{Cancel, Delta, Model, ModelError, ModelRequest, Reply, Secret, Thinking};
pub use super::openai::OpenAiCompatible;
use crate::session::{
    Backend, Capabilities, EventSink, ModelChoice, OpenRequest, PermissionMode, Session,
    SessionError, SessionId, SessionSummary,
};
use super::helpers::{anthropic_models, choice, env_key};

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

/// A model with no key. Every call says so, in words a person can act on.
struct MissingKey(pub(super) String);

impl Model for MissingKey {
    fn stream(&self, _: &ModelRequest<'_>, _: &mut dyn FnMut(Delta), _: &Cancel) -> Result<Reply, ModelError> {
        Err(ModelError::Auth(self.0.clone()))
    }
}

pub struct OwnAgent {
    pub(super) model: Arc<dyn Model>,
    pub(super) options: Arc<OwnOptions>,
    /// Why the agent cannot start, when it cannot.
    unavailable: Option<String>,
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
        Self::new(Arc::new(OpenAiCompatible::new(Some(key), super::openai::OPENAI_BASE)), options)
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
            default_mode: None,
            thinking: true,
            subagents: false,
            todos: false,
            providers: false,
            forks: false,
        }
    }

    fn open(&self, project: Arc<dyn Project>, request: OpenRequest, sink: EventSink) -> Result<Box<dyn Session>, SessionError> {
        if let Some(why) = &self.unavailable {
            return Err(SessionError::Start(why.clone()));
        }
        super::runner::open(self.model.clone(), self.options.clone(), project, request, sink)
    }

    /// One request, with no tools and no record: the model's text as it answers.
    fn draft(&self, _project: &dyn Project, prompt: &str, model: Option<&str>) -> Result<String, SessionError> {
        if let Some(why) = &self.unavailable {
            return Err(SessionError::Start(why.clone()));
        }
        let messages = [super::message::Message::user(prompt)];
        let request = super::message::ModelRequest {
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
                if let super::message::Delta::Text(more) = delta {
                    text.push_str(&more);
                }
            }, &super::message::Cancel::default())
            .map_err(|e| SessionError::Start(format!("{e:?}")))?;
        Ok(text.trim().to_string())
    }

    fn sessions(&self, project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
        super::store::list(project)
    }

    fn history(&self, project: &dyn Project, session: &SessionId) -> Result<Vec<crate::session::Event>, SessionError> {
        super::store::history(project, session)
    }
}
