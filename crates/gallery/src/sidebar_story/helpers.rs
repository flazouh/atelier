use atelier_ui::{
    Connection, Location, Need, ProjectData, SessionData, SessionStatus, agent_look::AgentLook,
};
use atelier_agents::claude;

use super::types::BASE;

pub(super) fn session(id: &str, title: &str, look: &AgentLook, status: SessionStatus, minutes_ago: u64) -> SessionData {
    SessionData { archived: false, in_panel: false, id: id.to_string().into(), title: title.to_string().into(), look: look.clone(), status, active_at: BASE - minutes_ago * 60 }
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
        location: Location::Ssh { host: "hp-agent".into() },
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
