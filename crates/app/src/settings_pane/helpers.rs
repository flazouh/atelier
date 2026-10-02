use gpui_kit::{AppContext, Hsla, Rgba};

/// The colour a byte triple names.
pub fn colour(bytes: [u8; 3]) -> Hsla {
    Rgba { r: bytes[0] as f32 / 255., g: bytes[1] as f32 / 255., b: bytes[2] as f32 / 255., a: 1. }.into()
}

/// The name a test finds a rule's switch by.
pub(crate) fn rule_switch(rule: atelier_tracker::Rule) -> &'static str {
    match rule {
        atelier_tracker::Rule::SessionStartMovesToInProgress => "rule-session-start",
        atelier_tracker::Rule::AgentFinishMovesToInReview => "rule-agent-finish",
        atelier_tracker::Rule::MergeMovesToDone => "rule-merge",
        atelier_tracker::Rule::SessionResumeMovesToInProgress => "rule-session-resume",
    }
}

/// Keeps a change, off the UI thread.
pub(super) fn save(cx: &mut gpui_kit::App, change: impl FnOnce(&mut atelier_settings::Settings) + Send + 'static) {
    // A test writes only the file it names, never this machine's settings.
    if cfg!(test) && std::env::var_os("ATELIER_SETTINGS").is_none() {
        return;
    }
    if let Some(path) = atelier_settings::path() {
        cx.background_spawn(async move {
            if let Err(error) = atelier_settings::update(&path, change) {
                eprintln!("could not save the settings: {error}");
            }
        })
        .detach();
    }
}
