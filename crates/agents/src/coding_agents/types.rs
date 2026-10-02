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
