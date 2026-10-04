use atelier_i18n::t;
use gpui_kit::{AppContext, Hsla, Rgba};

use super::strings as words;

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
pub(crate) fn save(cx: &mut gpui_kit::App, change: impl FnOnce(&mut atelier_settings::Settings) + Send + 'static) {
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

/// The body text's size at `zoom`, as the font size slider tells it.
pub fn font_size_words(zoom: f32) -> String {
    format!("{} pt", (14. * zoom).round() as i32)
}

/// The words of the switch for a part of a chip's card.
pub(crate) fn card_part_words(part: atelier_ui::PrPart) -> &'static str {
    use atelier_ui::PrPart;
    match part {
        PrPart::Branches => t(&words::PR_CARD_BRANCHES),
        PrPart::Failing => t(&words::PR_CARD_FAILING_CHECK),
        PrPart::Reviewers => t(&words::PR_CARD_REVIEWERS),
        PrPart::Merge => t(&words::PR_CARD_MERGE),
        PrPart::Sessions => t(&words::PR_CARD_SESSIONS),
        PrPart::Files => t(&words::PR_CARD_FILES),
        PrPart::Actions => t(&words::PR_CARD_ACTIONS),
        PrPart::Live => t(&words::PR_CARD_LIVE),
    }
}

/// The name a test finds the switch for a part of a chip's card by.
pub(crate) fn card_part_switch(part: atelier_ui::PrPart) -> &'static str {
    use atelier_ui::PrPart;
    match part {
        PrPart::Branches => "pr-card-branches",
        PrPart::Failing => "pr-card-failing-check",
        PrPart::Reviewers => "pr-card-reviewers",
        PrPart::Merge => "pr-card-merge",
        PrPart::Sessions => "pr-card-sessions",
        PrPart::Files => "pr-card-files",
        PrPart::Actions => "pr-card-actions",
        PrPart::Live => "pr-card-live",
    }
}
