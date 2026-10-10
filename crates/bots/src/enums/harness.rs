use serde::{Deserialize, Serialize};

/// The agent program a bot runs on. The names match the coding agents the app knows; the app maps them.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    ClaudeCode,
    Codex,
    Cursor,
    Grok,
    Opencode,
    Custom,
}
