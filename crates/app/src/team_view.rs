//! The Team view's data, from the sessions this window runs: the reader is the one person, and every open
//! session of every project is theirs. Teammates arrive here later, from a company server, as more
//! `Person`s and `TeamSession`s; the board already draws any number of lanes.
use std::collections::BTreeMap;

use beui::{
    project_badge,
    team_board_model::{Person, TeamSession},
};
use gpui_kit::{App, Entity};

use crate::{
    agents_view::{Badges, badge_of, project_id},
    open_project::OpenProject,
};

/// The reader's id on the board.
pub const ME: &str = "me";

/// The name the reader goes by: the login's name with a capital, else "You".
pub fn my_name() -> String {
    let login = std::env::var("USER").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| "You".into());
    let mut letters = login.chars();
    letters.next().map_or(login.clone(), |first| first.to_uppercase().chain(letters).collect())
}

/// The people on the board and their sessions. Each open session is the reader's, in its project, with
/// the project's badge colour.
pub fn data(projects: &[Entity<OpenProject>], badges: &Badges, name: &str, cx: &App) -> (Vec<Person>, Vec<TeamSession>) {
    let me = Person { id: ME.into(), name: name.to_string().into(), you: true, online: true, color: project_badge::fallback_color(name) };
    let places: Vec<(String, String)> = projects.iter().map(|p| (project_id(p.read(cx)).to_string(), p.read(cx).name())).collect();
    let pairs: Vec<(&str, &str)> = places.iter().map(|(place, name)| (place.as_str(), name.as_str())).collect();
    let labels: BTreeMap<String, String> = project_badge::labels(&pairs);
    let sessions = projects
        .iter()
        .flat_map(|project| {
            let p = project.read(cx);
            let place = project_id(p);
            let badge = badge_of(&place, labels.get(place.as_ref()).map_or("", String::as_str), badges);
            let project_name: gpui_kit::SharedString = p.name().into();
            p.sessions.iter().map(move |session| {
                let s = session.read(cx);
                TeamSession {
                    person: ME.into(),
                    project: project_name.clone(),
                    project_color: badge.color,
                    data: beui::sidebar_model::SessionData { id: s.key.clone(), title: s.shown_title(), look: s.agent.look.clone(), status: s.status.clone(), active_at: s.active_at },
                }
            })
        })
        .collect();
    (vec![me], sessions)
}
