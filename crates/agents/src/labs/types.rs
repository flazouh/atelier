use atelier_ui::BrandMark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lab {
    Anthropic,
    OpenAi,
    Xai,
    OpenRouter,
    GithubCopilot,
    Custom,
}

impl Lab {
    pub const ALL: [Lab; 6] = [Self::Anthropic, Self::OpenAi, Self::Xai, Self::OpenRouter, Self::GithubCopilot, Self::Custom];

    pub fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Xai => "xAI",
            Self::OpenRouter => "OpenRouter",
            Self::GithubCopilot => "GitHub Copilot",
            Self::Custom => "Custom",
        }
    }

    /// The lab's mark, or `None` for a monogram.
    pub fn mark(self) -> Option<BrandMark> {
        let pair = |name: &str| BrandMark::new(format!("labs/{name}-light.svg"), format!("labs/{name}-dark.svg"));
        match self {
            Self::Anthropic => Some(pair("anthropic")),
            Self::OpenAi => Some(pair("openai")),
            Self::OpenRouter => Some(pair("openrouter")),
            Self::GithubCopilot => Some(pair("copilot")),
            Self::Xai | Self::Custom => None,
        }
    }
}
