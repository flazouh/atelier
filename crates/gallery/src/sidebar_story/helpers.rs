use atelier_ui::{
    Connection, Location, Need, ProjectData, SessionData, SessionStatus, agent_look::AgentLook,
};
use atelier_agents::claude;

use super::types::BASE;

pub(super) fn session(id: &str, title: &str, look: &AgentLook, status: SessionStatus, minutes_ago: u64) -> SessionData {
    SessionData { archived: false, in_panel: false, provider: None, id: id.to_string().into(), title: title.to_string().into(), look: look.clone(), status, active_at: BASE - minutes_ago * 60 }
}

/// Where a session can be handed off to, as the app gives it: Claude Code opens its accounts and OpenRouter, the
/// other agents hand off at once.
pub(super) fn handoff_targets() -> Vec<atelier_ui::menu::Branch> {
    use atelier_agents::{coding_agents::CodingAgent, labs::Lab};
    use atelier_ui::menu::{Branch, Lead};
    let anthropic = || Lead::of(Lab::Anthropic.mark());
    vec![
        Branch::with(
            "claude-code",
            "Claude Code",
            vec![
                Branch::leaf("claude-code/account:default", "Max").led(anthropic()),
                Branch::leaf("claude-code/account:work", "Max · work").led(anthropic()),
                Branch::leaf("claude-code/openrouter", "OpenRouter").led(Lead::of(Lab::OpenRouter.mark())),
            ],
        )
        .led(Lead::of(CodingAgent::ClaudeCode.mark())),
        Branch::leaf("cursor", "Cursor").led(Lead::of(CodingAgent::Cursor.mark())),
        Branch::leaf("codex", "Codex").led(Lead::of(CodingAgent::Codex.mark())),
    ]
}

pub(super) fn sample(other: &AgentLook) -> Vec<ProjectData> {
    let claude = claude::look();
    let atelier = ProjectData {
        id: "atelier".into(),
        name: "atelier".into(),
        location: Location::Local,
        connection: Connection::Connected,
        pulls_unavailable: None,
        badge: Default::default(),
        sessions: vec![
            session("l1", "Add the sidebar and the agent panels", &claude, SessionStatus::Working, 0),
            session("l2", "Why does the diff layout miss 120Hz on the Mac?", &claude, SessionStatus::NeedsYou(Need::Approval), 3),
            session("l3", "Port GitQuiet's Court sorting into a pure module", &claude, SessionStatus::Finished, 12),
            session("l4", "Make the forge headless", other, SessionStatus::Idle, 60),
            session("l5", "Fix the flaky app test", &claude, SessionStatus::Failed("the process exited with code 3".into()), 95),
            session("l6", "Review the M3 core", other, SessionStatus::Idle, 180),
            session("l7", "Rename Court to Shelf everywhere", &claude, SessionStatus::Idle, 300),
            session("l8", "Explain the highlight cache", &claude, SessionStatus::Idle, 1500),
            session("l9", "Try a different spring for the layout", other, SessionStatus::Idle, 4000),
        ],
    };
    let api = ProjectData {
        id: "api".into(),
        name: "api-server".into(),
        location: Location::Ssh { host: "dev-host".into() },
        connection: Connection::Connected,
        pulls_unavailable: None,
        badge: Default::default(),
        sessions: vec![
            session("a1", "Which endpoints still return 500?", &claude, SessionStatus::NeedsYou(Need::Question), 1),
            session("a2", "Migrate the sessions table", &claude, SessionStatus::Finished, 25),
            session("a3", "Write the OpenAPI notes", other, SessionStatus::Idle, 240),
        ],
    };
    let infra = ProjectData {
        id: "infra".into(),
        name: "infra".into(),
        location: Location::Ssh { host: "build-01".into() },
        connection: Connection::Reconnecting,
        pulls_unavailable: None,
        badge: Default::default(),
        sessions: vec![],
    };
    vec![atelier, api, infra]
}

/// `projects` projects of `sessions` sessions, in all the statuses.
pub(super) fn big(projects: usize, sessions: usize) -> Vec<ProjectData> {
    let claude = claude::look();
    (0..projects)
        .map(|p| ProjectData {
            id: format!("p{p}").into(),
            name: format!("project-{p}").into(),
            location: if p % 3 == 0 { Location::Ssh { host: format!("host-{}", p % 5).into() } } else { Location::Local },
            connection: Connection::Connected,
            pulls_unavailable: None,
        badge: Default::default(),
            sessions: (0..sessions)
                .map(|s| {
                    let status = match s % 9 {
                        0 => SessionStatus::Working,
                        1 => SessionStatus::NeedsYou(Need::Approval),
                        2 => SessionStatus::Finished,
                        3 => SessionStatus::Failed("stopped".into()),
                        _ => SessionStatus::Idle,
                    };
                    session(&format!("p{p}s{s}"), &format!("A session about something, number {s}"), &claude, status, (s as u64 + 1) * 7)
                })
                .collect(),
        })
        .collect()
}
