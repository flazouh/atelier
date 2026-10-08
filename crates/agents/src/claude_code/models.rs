//! The models Anthropic offers the Claude sign-in, asked of its API as Claude Code asks it, so the picker names the real versions
//! ("Opus 5.5") and a release needs no change here.
use atelier_project::Project;
use serde_json::Value;

use crate::{session::ModelChoice, usage::ClaudeUsage};

const URL: &str = "https://api.anthropic.com/v1/models?limit=100";
const BETA: &str = "oauth-2025-04-20";
const VERSION: &str = "2023-06-01";
const TIMEOUT_SECS: u64 = 10;

/// The models in the endpoint's answer, newest first as it sends them: the id to start the agent with, and the name without
/// its "Claude " (the picker is the agent's already).
pub(super) fn parse(text: &str) -> Result<Vec<ModelChoice>, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| "the models could not be read".to_string())?;
    let models: Vec<ModelChoice> = value
        .get("data")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|m| {
                    let id = m.get("id")?.as_str()?;
                    let name = m.get("display_name").and_then(Value::as_str).unwrap_or(id);
                    Some(ModelChoice { id: id.to_string(), label: name.strip_prefix("Claude ").unwrap_or(name).to_string() })
                })
                .collect()
        })
        .unwrap_or_default();
    if models.is_empty() { Err("Anthropic listed no models".into()) } else { Ok(models) }
}

/// Asks for the models, with the sign-in of `project`'s host.
pub(super) fn fetch(project: &dyn Project, now: i64) -> Result<Vec<ModelChoice>, String> {
    let token = ClaudeUsage::access_token(project, now)?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(TIMEOUT_SECS))).build().into();
    let mut answer = agent
        .get(URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", BETA)
        .header("anthropic-version", VERSION)
        .call()
        .map_err(|e| format!("the models could not be reached: {e}"))?;
    parse(&answer.body_mut().read_to_string().map_err(|e| format!("the models could not be read: {e}"))?)
}
