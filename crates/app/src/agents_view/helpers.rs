use std::{collections::BTreeMap, path::PathBuf};

use atelier_ui::{
    panel_types::{PanelData, ProjectLabel},
    project_badge,
    session_status::SessionStatus,
    sidebar_model::{Badge, Connection, Location as RowLocation, ProjectData, SessionData},
};
use gpui_kit::{App, Entity, SharedString};
use atelier_agents::session::SessionId;
use atelier_project::Link;
use atelier_settings::Location;

use crate::{agent_session::AgentSession, open_project::OpenProject};
use super::structs::Badges;
use super::types::{PAST, Pick};

pub fn pick(id: &str) -> Pick {
    match id.strip_prefix(PAST) {
        Some(past) => Pick::Past(SessionId(past.to_string())),
        None => Pick::Open(id.to_string().into()),
    }
}

pub fn project_id(project: &OpenProject) -> SharedString {
    project.location.place().into()
}

pub(super) fn label(project: &OpenProject) -> ProjectLabel {
    ProjectLabel { id: project_id(project), name: project.name().into(), location: row_location(&project.location) }
}

fn row_location(location: &Location) -> RowLocation {
    match location {
        Location::Local { .. } => RowLocation::Local,
        Location::Ssh { host, .. } => RowLocation::Ssh { host: host.clone().into() },
    }
}

/// The badge of the project at `place`: the label the set gave it, the colour the reader picked or its place gives,
/// and its image when the file is still there.
pub fn badge_of(place: &str, label: &str, badges: &Badges) -> Badge {
    Badge {
        label: label.to_string().into(),
        color: project_badge::color_of(badges.colors.get(place).map(|c| usize::from(*c)), place),
        icon: badges.icons.get(place).map(PathBuf::from).filter(|path| path.exists()),
    }
}

/// Every project with its sessions, open ones first as the agent names them, then past ones.
pub fn sidebar(projects: &[Entity<OpenProject>], names: &BTreeMap<String, String>, badges: &Badges, archived: &std::collections::BTreeSet<String>, cx: &App) -> Vec<ProjectData> {
    let places: Vec<(String, String)> = projects.iter().map(|p| (project_id(p.read(cx)).to_string(), p.read(cx).name())).collect();
    let pairs: Vec<(&str, &str)> = places.iter().map(|(place, name)| (place.as_str(), name.as_str())).collect();
    let labels = project_badge::labels(&pairs);
    projects
        .iter()
        .map(|p| {
            let p = p.read(cx);
            let open = p.sessions.iter().map(|s| {
                let s = s.read(cx);
                SessionData {
                    in_panel: true,
                    archived: s.id.as_ref().is_some_and(|id| archived.contains(id.as_str())),
                    id: s.key.clone(),
                    title: s.shown_title(),
                    look: s.agent.look.clone(),
                    status: s.status.clone(),
                    active_at: s.active_at,
                }
            });
            let past = p.past.iter().map(|past| SessionData {
                in_panel: false,
                archived: archived.contains(&past.id.0),
                id: format!("{PAST}{}", past.id.0).into(),
                title: names.get(&past.id.0).cloned().unwrap_or_else(|| past.title.clone()).into(),
                look: p.agent.look.clone(),
                status: SessionStatus::Idle,
                active_at: past.updated.unwrap_or(0),
            });
            ProjectData {
                id: project_id(p),
                name: p.name().into(),
                location: row_location(&p.location),
                connection: match &p.link {
                    Link::Up => Connection::Connected,
                    Link::Down(_) => Connection::Reconnecting,
                },
                branch: p.git.branch().cloned(),
                sessions: open.chain(past).collect(),
                pulls_unavailable: p.pulls_unavailable().map(SharedString::from),
                tasks_open: p.tasks_open(),
                badge: {
                    let place = project_id(p);
                    badge_of(&place, labels.get(place.as_ref()).map_or("", String::as_str), badges)
                },
            }
        })
        .collect()
}

/// A panel for each open session, and the projects' order for grouping.
/// The open sessions as panels. `view_of` gives each session's own view (`SessionPanel`), kept across syncs.
pub fn panels(
    projects: &[Entity<OpenProject>],
    view_of: &dyn Fn(&Entity<AgentSession>) -> gpui_kit::AnyView,
    cx: &App,
) -> (Vec<PanelData>, Vec<SharedString>) {
    let order = projects.iter().map(|p| project_id(p.read(cx))).collect();
    let panels = projects
        .iter()
        .flat_map(|p| {
            let project = label(p.read(cx));
            p.read(cx).sessions.clone().into_iter().map(move |session| (project.clone(), session))
        })
        .map(|(project, session)| {
            let s = session.read(cx);
            PanelData {
                id: s.key.clone(),
                project,
                title: s.shown_title(),
                look: s.agent.look.clone(),
                status: s.status.clone(),
                content: view_of(&session),
            }
        })
        .collect();
    (panels, order)
}
