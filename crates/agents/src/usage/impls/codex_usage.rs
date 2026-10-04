use atelier_project::{Command, Project};
use serde_json::Value;

use crate::subprocess;

use super::super::{
    consts::{CODEX_READ_ID, CODEX_SCRIPT, DAY, HOUR, MINUTE, SHELL},
    structs::{CodexUsage, Reading},
    traits::UsageSource,
};

impl CodexUsage {
    /// Reads Codex's `codex` program, as the project finds it.
    pub fn new(program: impl Into<String>) -> Self {
        Self { program: program.into() }
    }

    /// Finds the answer to the rate-limit request among the lines the app server wrote.
    pub(in super::super) fn parse(output: &str, now: i64) -> Result<Reading, String> {
        let answer = output
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v.get("id").and_then(Value::as_i64) == Some(CODEX_READ_ID))
            .ok_or("Codex did not answer: is it installed and signed in?")?;
        if let Some(error) = answer.get("error") {
            let words = error.get("message").and_then(Value::as_str).unwrap_or("Codex refused the request");
            return Err(words.to_string());
        }
        let limits = answer.pointer("/result/rateLimits").ok_or("Codex reported no limits")?;
        let mut reading = Reading::default();
        for key in ["primary", "secondary"] {
            let Some(window) = limits.get(key).filter(|w| w.is_object()) else { continue };
            let percent = window.get("usedPercent").and_then(Value::as_f64).unwrap_or(0.);
            let label = window.get("windowDurationMins").and_then(Value::as_u64).map(span).unwrap_or_else(|| key.into());
            let resets_in = window.get("resetsAt").and_then(Value::as_i64).map(|at| at.saturating_sub(now).max(0) as u64);
            reading = reading.window(label, percent, resets_in);
        }
        if reading.windows.is_empty() {
            return Err("Codex reported no limits".into());
        }
        Ok(reading.note(limits.get("planType").and_then(Value::as_str).map(|plan| format!("Codex {plan} plan"))))
    }
}

impl UsageSource for CodexUsage {
    fn name(&self) -> &str {
        "Codex"
    }

    fn read(&self, project: &dyn Project, now: i64) -> Result<Reading, String> {
        let command = Command::new(SHELL).args(["-c", CODEX_SCRIPT, &self.program]);
        let output = subprocess::output(project, &command).map_err(|e| format!("Codex could not be reached: {e}"))?;
        Self::parse(&output, now)
    }
}

/// A window of `minutes`, in the biggest unit it fills evenly: 300 is `5h`, 10080 is `7d`, 43200 is `30d`.
fn span(minutes: u64) -> String {
    let seconds = minutes * MINUTE;
    if seconds.is_multiple_of(DAY) {
        format!("{}d", seconds / DAY)
    } else if seconds.is_multiple_of(HOUR) {
        format!("{}h", seconds / HOUR)
    } else {
        format!("{minutes}m")
    }
}
