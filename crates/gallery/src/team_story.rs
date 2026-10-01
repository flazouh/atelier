//! The "Team" story: the team board with four people and a dozen sessions in every status, so the lanes,
//! the order inside a lane, and the summary can be looked at. The data is fixture data; the app fills the
//! board from the sessions it runs.
use beui::{
    AgentLook,
    session_status::{Need, SessionStatus},
    sidebar_model::SessionData,
    team_board::TeamBoard,
    team_board_model::{Person, TeamSession},
};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use lathe_agents::claude;

pub struct TeamStory {
    look: AgentLook,
    people: Vec<Person>,
    sessions: Vec<TeamSession>,
}

const NOW: u64 = 1_800_000_000;

impl TeamStory {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        let look = claude::look();
        let person = |id: &str, name: &str, you, online, color| Person { id: id.into(), name: name.into(), you, online, color };
        let people = vec![
            person("me", "Alex", true, true, 3),
            person("bea", "Bea", false, true, 7),
            person("dan", "Dan", false, false, 0),
            person("eli", "Eli", false, true, 10),
        ];
        let session = |who: &str, project: &str, color: usize, title: &str, status: SessionStatus, ago: u64| TeamSession {
            person: who.into(),
            project: project.into(),
            project_color: color,
            data: SessionData { id: format!("{who}-{title}").into(), title: title.into(), look: look.clone(), status, active_at: NOW - ago },
        };
        let sessions = vec![
            session("me", "lathe", 1, "Wire the / list into the composer", SessionStatus::Working, 10),
            session("me", "lathe", 1, "Team view, lanes for each person", SessionStatus::NeedsYou(Need::Approval), 120),
            session("me", "fluentai", 5, "Fix the YouTube start time", SessionStatus::Finished, 900),
            session("me", "fluentai", 5, "Tutor core on Effect", SessionStatus::Idle, 7200),
            session("bea", "fluentai", 5, "Billing: entitlements and checkout", SessionStatus::Working, 30),
            session("bea", "fluentai", 5, "Which plan does a trial user get? Needs a decision", SessionStatus::NeedsYou(Need::Question), 300),
            session("bea", "docs", 9, "Rewrite the onboarding guide", SessionStatus::Idle, 86_400),
            session("dan", "ori", 2, "Codex compatibility matrix", SessionStatus::Idle, 172_800),
            session("dan", "ori", 2, "Harness probe stopped", SessionStatus::Failed("exit 1".into()), 3600),
            session("eli", "books", 4, "Import the March statements", SessionStatus::Working, 5),
            session("eli", "books", 4, "Reconcile the card feed", SessionStatus::Finished, 600),
        ];
        Self { look, people, sessions }
    }
}

impl Render for TeamStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.look;
        div().size_full().h(gpui_kit::px(640.)).child(TeamBoard::new("team-story", self.people.clone(), self.sessions.clone(), NOW).on_open(|id, _, _| println!("open {id}")))
    }
}
