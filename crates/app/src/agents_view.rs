//! The window's projects and sessions as the sidebar and the agent panels take them: plain data, made
//! fresh from the open projects whenever a session or a project changes.
//!
//! A session open in the window is keyed by its panel's key. A past one, which the agent lists but no
//! panel holds, is keyed `past:<the agent's id>`, so opening it resumes it.

use std::{collections::BTreeMap, rc::Rc};

use beui::{
    panel_types::{PanelData, ProjectLabel},
    session_status::SessionStatus,
    sidebar_model::{Connection, Location as RowLocation, ProjectData, SessionData},
};
use gpui_kit::{App, Entity, IntoElement, SharedString};
use lathe_agents::session::SessionId;
use lathe_project::Link;
use lathe_settings::Location;

use crate::{
    open_project::OpenProject,
    session_view::session_view,
};

const PAST: &str = "past:";

/// What a sidebar row or a panel names.
pub enum Pick {
    /// A session open in the window, by its key.
    Open(SharedString),
    /// A past session, to resume.
    Past(SessionId),
}

pub fn pick(id: &str) -> Pick {
    match id.strip_prefix(PAST) {
        Some(past) => Pick::Past(SessionId(past.to_string())),
        None => Pick::Open(id.to_string().into()),
    }
}

pub fn project_id(project: &OpenProject) -> SharedString {
    project.location.place().into()
}

fn label(project: &OpenProject) -> ProjectLabel {
    ProjectLabel { id: project_id(project), name: project.name().into(), location: row_location(&project.location) }
}

fn row_location(location: &Location) -> RowLocation {
    match location {
        Location::Local { .. } => RowLocation::Local,
        Location::Ssh { host, .. } => RowLocation::Ssh { host: host.clone().into() },
    }
}

/// Every project with its sessions, open ones first as the agent names them, then past ones.
pub fn sidebar(projects: &[Entity<OpenProject>], names: &BTreeMap<String, String>, cx: &App) -> Vec<ProjectData> {
    projects
        .iter()
        .map(|p| {
            let p = p.read(cx);
            let open = p.sessions.iter().map(|s| {
                let s = s.read(cx);
                SessionData {
                    id: s.key.clone(),
                    title: s.shown_title(),
                    look: s.agent.look.clone(),
                    status: s.status.clone(),
                    active_at: s.active_at,
                }
            });
            let past = p.past.iter().map(|past| SessionData {
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
            }
        })
        .collect()
}

/// A panel for each open session, and the projects' order for grouping.
pub fn panels(projects: &[Entity<OpenProject>], cx: &App) -> (Vec<PanelData>, Vec<SharedString>) {
    let order = projects.iter().map(|p| project_id(p.read(cx))).collect();
    let panels = projects
        .iter()
        .flat_map(|p| {
            let project = label(p.read(cx));
            p.read(cx).sessions.clone().into_iter().map(move |session| (project.clone(), session))
        })
        .map(|(project, session)| {
            let s = session.read(cx);
            let content = session.clone();
            PanelData {
                id: s.key.clone(),
                project,
                title: s.shown_title(),
                look: s.agent.look.clone(),
                status: s.status.clone(),
                content: Rc::new(move |window, cx| session_view(&content, window, cx).into_any_element()),
            }
        })
        .collect();
    (panels, order)
}
