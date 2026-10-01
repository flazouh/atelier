//! The coding agents atelier can drive, and their marks, as acepe keeps them: Claude Code, Codex, Cursor,
//! Grok, opencode, and Custom, which takes atelier-ui's monogram.

use atelier_ui::BrandMark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodingAgent {
    ClaudeCode,
    Codex,
    Cursor,
    Grok,
    Opencode,
    Custom,
}

impl CodingAgent {
    pub const ALL: [CodingAgent; 6] = [Self::ClaudeCode, Self::Codex, Self::Cursor, Self::Grok, Self::Opencode, Self::Custom];

    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Grok => "Grok",
            Self::Opencode => "opencode",
            Self::Custom => "Custom",
        }
    }

    pub fn mark(self) -> Option<BrandMark> {
        let pair = |name: &str| BrandMark::new(format!("coding/{name}-light.svg"), format!("coding/{name}-dark.svg"));
        match self {
            Self::ClaudeCode => Some(pair("claude")),
            Self::Codex => Some(pair("codex")),
            Self::Cursor => Some(pair("cursor")),
            Self::Grok => Some(pair("grok")),
            Self::Opencode => Some(pair("opencode")),
            Self::Custom => None,
        }
    }
}

pub(crate) fn bytes(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "coding/claude-light.svg" => include_bytes!("../assets/coding/claude-light.svg"),
        "coding/claude-dark.svg" => include_bytes!("../assets/coding/claude-dark.svg"),
        "coding/codex-light.svg" => include_bytes!("../assets/coding/codex-light.svg"),
        "coding/codex-dark.svg" => include_bytes!("../assets/coding/codex-dark.svg"),
        "coding/cursor-light.svg" => include_bytes!("../assets/coding/cursor-light.svg"),
        "coding/cursor-dark.svg" => include_bytes!("../assets/coding/cursor-dark.svg"),
        "coding/grok-light.svg" => include_bytes!("../assets/coding/grok-light.svg"),
        "coding/grok-dark.svg" => include_bytes!("../assets/coding/grok-dark.svg"),
        "coding/opencode-light.svg" => include_bytes!("../assets/coding/opencode-light.svg"),
        "coding/opencode-dark.svg" => include_bytes!("../assets/coding/opencode-dark.svg"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
