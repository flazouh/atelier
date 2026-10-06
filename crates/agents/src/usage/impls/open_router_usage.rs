use atelier_project::Project;
use serde_json::Value;

use super::super::{
    consts::{CLAUDE_TIMEOUT_SECS, OPENROUTER_URL},
    structs::{OpenRouterUsage, Reading},
    traits::UsageSource,
};

impl OpenRouterUsage {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }

    /// Reads `/api/v1/key`: with a limit on the key, one window of credit; without, only the spend, as a note.
    pub(in super::super) fn parse(text: &str) -> Result<Reading, String> {
        let value: Value = serde_json::from_str(text).map_err(|_| "OpenRouter's answer could not be read".to_string())?;
        let data = value.get("data").ok_or("OpenRouter's answer had no key in it")?;
        let spent = data.get("usage").and_then(Value::as_f64).unwrap_or(0.);
        let Some(limit) = data.get("limit").and_then(Value::as_f64).filter(|limit| *limit > 0.) else {
            return Ok(Reading::default().note(Some(format!("${spent:.2} spent, no limit on the key"))));
        };
        // `usage` is all the key ever spent, and a limit that resets is spent from its own start: the credit left is the
        // truth about the window. Without it, the spend is all there is.
        let used = data.get("limit_remaining").and_then(Value::as_f64).map_or(spent, |left| (limit - left).max(0.));
        Ok(Reading::default().window("credit", used / limit * 100., None).note(Some(format!("${used:.2} of ${limit:.2} credit"))))
    }

    fn fetch(&self) -> Result<String, String> {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(CLAUDE_TIMEOUT_SECS))).build().into();
        match agent.get(OPENROUTER_URL).header("Authorization", format!("Bearer {}", self.key)).call() {
            Ok(mut answer) => answer.body_mut().read_to_string().map_err(|e| format!("OpenRouter's answer could not be read: {e}")),
            Err(ureq::Error::StatusCode(401 | 403)) => Err("OpenRouter does not know this key".into()),
            Err(error) => Err(format!("OpenRouter could not be reached: {error}")),
        }
    }
}

impl UsageSource for OpenRouterUsage {
    fn name(&self) -> &str {
        "OpenRouter"
    }

    /// Asks from this machine: the key is here, not on the project's host.
    fn read(&self, _: &dyn Project, _: i64) -> Result<Reading, String> {
        Self::parse(&self.fetch()?)
    }
}
