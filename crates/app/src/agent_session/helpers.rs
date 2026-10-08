use std::time::SystemTime;

use crate::palette::Hue;
use atelier_agents::session::{Event, PermissionMode, SessionError, TurnOutcome};

/// Whether `event` is the agent at work, which stamps the session's row: its start on a resume is not.
pub(super) fn is_activity(event: &Event) -> bool {
    !matches!(event, Event::Started(_))
}

/// Whether `event` ends a turn that went to its end. A queued message waits out a Stop or a failure, so
/// the person decides what comes next.
pub(super) fn completes_turn(event: &Event) -> bool {
    matches!(event, Event::TurnEnded(end) if end.outcome == TurnOutcome::Completed)
}

/// Seconds since the Unix epoch, for "2m ago".
pub fn now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The permission modes as the mode picker names them.
pub fn mode_word(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "Ask first",
        PermissionMode::AcceptEdits => "Accept edits",
        PermissionMode::Plan => "Plan",
        PermissionMode::Auto => "Auto",
        PermissionMode::Bypass => "Bypass permissions",
    }
}

/// A small icon for each mode and its colour, from the palette: a question for asking first, a pencil for edits, a list for the plan, a
/// bulb for auto, a warning for no checks at all.
pub fn mode_look(mode: PermissionMode) -> (atelier_ui::IconName, gpui_kit::Hsla) {
    let (icon, hue) = match mode {
        PermissionMode::Ask => (atelier_ui::IconName::Help, Hue::Blue),
        PermissionMode::AcceptEdits => (atelier_ui::IconName::Edit, Hue::Green),
        PermissionMode::Plan => (atelier_ui::IconName::Checklist, Hue::Purple),
        PermissionMode::Auto => (atelier_ui::IconName::Idea, Hue::Orange),
        PermissionMode::Bypass => (atelier_ui::IconName::Warning, Hue::Red),
    };
    (icon, hue.hsla())
}
pub(super) fn mode_from(word: &str) -> Option<PermissionMode> {
    [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Auto, PermissionMode::Bypass]
        .into_iter()
        .find(|m| mode_word(*m) == word)
}

/// A start that failed, in words that say what to do.
pub(super) fn problem_words(error: &SessionError) -> String {
    match error {
        SessionError::Missing { program } => {
            format!("{program} is not installed on this host. Install it there, then start a new session.")
        }
        other => other.to_string(),
    }
}
