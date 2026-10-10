use atelier_bot_face::Mood;
use atelier_bots::{Access, Harness, Provider, Voice};

/// The harness, as the app names it.
pub fn harness_words(harness: Harness) -> &'static str {
    match harness {
        Harness::ClaudeCode => "Claude Code",
        Harness::Codex => "Codex",
        Harness::Cursor => "Cursor",
        Harness::Grok => "Grok",
        Harness::Opencode => "OpenCode",
        Harness::Custom => "Custom",
    }
}

/// The provider and its model; the harness picks the model when the bot names none.
pub fn provider_words(provider: &Provider) -> String {
    match &provider.model {
        Some(model) => format!("{} · {model}", provider.service),
        None => format!("{} · the harness picks the model", provider.service),
    }
}

pub fn voice_words(voice: Voice) -> &'static str {
    match voice {
        Voice::CalmAndClear => "Calm and clear",
        Voice::Cheerful => "Cheerful",
        Voice::ShortAndDry => "Short and dry",
        Voice::Thorough => "Thorough",
        Voice::Playful => "Playful",
    }
}

pub fn access_words(access: Access) -> &'static str {
    match access {
        Access::Read => "read",
        Access::Write => "write, asks each time",
    }
}

/// The mood, as the mood switch names it.
pub fn mood_words(mood: Mood) -> &'static str {
    match mood {
        Mood::Idle => "Idle",
        Mood::Thinking => "Thinking",
        Mood::Working => "Working",
        Mood::Done => "Done",
        Mood::Needs => "Needs you",
        Mood::Stuck => "Stuck",
    }
}
