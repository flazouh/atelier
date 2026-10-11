use atelier_bot_face::Mood;
use atelier_ui::session_status::SessionStatus;

/// The mood a bot's face shows for what its session does, as `docs/bots/faces-v1.md` maps them: idle while it waits,
/// thinking while the agent plans, working while a tool runs, done when a turn ended and the reader has not looked,
/// needs you for an approval or a question, and stuck after a failure. `tool_runs` tells planning from running a tool.
pub fn mood_of(status: &SessionStatus, tool_runs: bool) -> Mood {
    match status {
        SessionStatus::Idle => Mood::Idle,
        SessionStatus::Working if tool_runs => Mood::Working,
        SessionStatus::Working => Mood::Thinking,
        SessionStatus::Finished => Mood::Done,
        SessionStatus::NeedsYou(_) => Mood::Needs,
        SessionStatus::Failed(_) => Mood::Stuck,
    }
}
