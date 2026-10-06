use atelier_project::{Command, Project};
use chrono::DateTime;
use serde_json::Value;

use crate::subprocess;

use super::super::{
    consts::{CLAUDE_BETA, CLAUDE_CREDENTIALS, CLAUDE_TIMEOUT_SECS, CLAUDE_URL, SHELL},
    structs::{ClaudeUsage, Reading},
    traits::UsageSource,
};

/// The windows Claude reports, with the label each shows under.
const WINDOWS: [(&str, &str); 4] =
    [("five_hour", "5h"), ("seven_day", "7d"), ("seven_day_opus", "Opus 7d"), ("seven_day_sonnet", "Sonnet 7d")];

impl ClaudeUsage {
    /// The access token in the sign-in `credentials` holds, unless it has run out at `now`.
    pub(in super::super) fn token(credentials: &str, now: i64) -> Result<String, String> {
        let signed_in = || "Claude is not signed in on this machine".to_string();
        let value: Value = serde_json::from_str(credentials.trim()).map_err(|_| signed_in())?;
        let oauth = value.get("claudeAiOauth").ok_or_else(signed_in)?;
        let token = oauth.get("accessToken").and_then(Value::as_str).filter(|t| !t.is_empty()).ok_or_else(signed_in)?;
        let expired = oauth.get("expiresAt").and_then(Value::as_i64).is_some_and(|at_ms| at_ms / 1000 <= now);
        if expired {
            return Err("Claude's sign-in has run out: open Claude once to renew it".into());
        }
        Ok(token.to_string())
    }

    /// Reads the endpoint's answer.
    pub(in super::super) fn parse(text: &str, now: i64) -> Result<Reading, String> {
        let value: Value = serde_json::from_str(text).map_err(|_| "Claude's usage could not be read".to_string())?;
        let mut reading = Reading::default();
        for (key, label) in WINDOWS {
            let Some(window) = value.get(key).filter(|w| w.is_object()) else { continue };
            let percent = window.get("utilization").and_then(Value::as_f64).unwrap_or(0.);
            reading = reading.window(label, percent, resets_in(window.get("resets_at"), now));
        }
        if reading.windows.is_empty() {
            return Err("Claude reported no limits".into());
        }
        Ok(reading.note(extra_usage(value.get("extra_usage"))))
    }

    /// Why the usage call failed, in words that say what to do: a refused sign-in is renewed in Claude, and a call that was
    /// asked for too often comes back by itself.
    pub(in super::super) fn fetch_failed(error: &ureq::Error) -> String {
        match error {
            ureq::Error::StatusCode(401 | 403) => "Claude's sign-in was refused: open Claude once to renew it".into(),
            ureq::Error::StatusCode(429) => "Claude's usage was asked for too often: it shows again in a few minutes".into(),
            error => format!("Claude's usage could not be reached: {error}"),
        }
    }
    fn fetch(token: &str) -> Result<String, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(CLAUDE_TIMEOUT_SECS)))
            .build()
            .into();
        let answer = agent.get(CLAUDE_URL).header("Authorization", format!("Bearer {token}")).header(CLAUDE_BETA.0, CLAUDE_BETA.1).call();
        match answer {
            Ok(mut answer) => answer.body_mut().read_to_string().map_err(|e| format!("Claude's usage could not be read: {e}")),
            Err(error) => Err(Self::fetch_failed(&error)),
        }
    }
}

impl UsageSource for ClaudeUsage {
    fn name(&self) -> &str {
        "Claude"
    }

    fn read(&self, project: &dyn Project, now: i64) -> Result<Reading, String> {
        let command = Command::new(SHELL).args(["-c", CLAUDE_CREDENTIALS]);
        let credentials = subprocess::output(project, &command).map_err(|e| format!("Claude could not be reached: {e}"))?;
        let token = Self::token(&credentials, now)?;
        Self::parse(&Self::fetch(&token)?, now)
    }
}

/// Seconds from `now` to the RFC 3339 time `at`; none when it is missing, not a time, or past.
fn resets_in(at: Option<&Value>, now: i64) -> Option<u64> {
    let at = DateTime::parse_from_rfc3339(at?.as_str()?).ok()?.timestamp();
    Some(at.saturating_sub(now).max(0) as u64)
}

/// `Extra usage: $100.03 of $100.00` when the plan has spend beyond it switched on.
fn extra_usage(extra: Option<&Value>) -> Option<String> {
    let extra = extra.filter(|e| e.get("is_enabled").and_then(Value::as_bool) == Some(true))?;
    let places = extra.get("decimal_places").and_then(Value::as_i64).unwrap_or(2).clamp(0, 6) as usize;
    let scale = 10f64.powi(places as i32);
    let used = extra.get("used_credits").and_then(Value::as_f64)? / scale;
    let limit = extra.get("monthly_limit").and_then(Value::as_f64)? / scale;
    let sign = match extra.get("currency").and_then(Value::as_str) {
        Some("USD") | None => "$".to_string(),
        Some(other) => format!("{other} "),
    };
    Some(format!("Extra usage: {sign}{used:.places$} of {sign}{limit:.places$}"))
}
