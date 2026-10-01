//! The labs that make models, and their marks, as acepe keeps them (`provider-brand-icons.ts`,
//! `upstream-provider-mark.svelte`): Anthropic, OpenAI, xAI, OpenRouter, GitHub Copilot, and Custom. acepe
//! has no mark for xAI, so it and Custom take atelier-ui's monogram.

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

/// The embedded mark at an asset path.
pub(crate) fn bytes(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "labs/anthropic-light.svg" => include_bytes!("../assets/labs/anthropic-light.svg"),
        "labs/anthropic-dark.svg" => include_bytes!("../assets/labs/anthropic-dark.svg"),
        "labs/openai-light.svg" => include_bytes!("../assets/labs/openai-light.svg"),
        "labs/openai-dark.svg" => include_bytes!("../assets/labs/openai-dark.svg"),
        "labs/openrouter-light.svg" => include_bytes!("../assets/labs/openrouter-light.svg"),
        "labs/openrouter-dark.svg" => include_bytes!("../assets/labs/openrouter-dark.svg"),
        "labs/copilot-light.svg" => include_bytes!("../assets/labs/copilot-light.svg"),
        "labs/copilot-dark.svg" => include_bytes!("../assets/labs/copilot-dark.svg"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
