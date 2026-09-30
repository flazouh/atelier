//! The "Sidebar" story: three projects (one on this machine, one over SSH and connected, one over SSH and
//! reconnecting), and sessions in every status. "Live" cycles the statuses so a row can be watched moving,
//! fading its mark and entering. `GALLERY_SCROLL=1` scrolls a sidebar of `SIDEBAR_PROJECTS` projects
//! (50 by default) of `SIDEBAR_SESSIONS` sessions each (40) and prints the frame numbers.
use std::time::{Duration, Instant};

use beui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, Connection, Location, Need, ProjectData, SessionData, SessionStatus, Sidebar,
    SidebarEvent, agent_look::AgentLook,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window, div, px,
};
use lathe_agents::claude;

pub mod run;
use run::Run;

/// The time the story starts at, in seconds since the Unix epoch.
const BASE: u64 = 1_790_700_000;
/// How long a live step lasts.
const STEP: Duration = Duration::from_millis(1800);

fn session(id: &str, title: &str, look: &AgentLook, status: SessionStatus, minutes_ago: u64) -> SessionData {
    SessionData { id: id.to_string().into(), title: title.to_string().into(), look: look.clone(), status, active_at: BASE - minutes_ago * 60 }
}

fn sample(other: &AgentLook) -> Vec<ProjectData> {
    let claude = claude::look();
    let lathe = ProjectData {
        id: "lathe".into(),
        name: "lathe".into(),
        location: Location::Local,
        connection: Connection::Connected,
        branch: Some("main".into()),
        pulls_unavailable: None,
        tasks_open: None,
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
        branch: Some("feature/tokens".into()),
        pulls_unavailable: None,
        tasks_open: None,
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
        branch: None,
        pulls_unavailable: None,
        tasks_open: None,
        sessions: vec![],
    };
    vec![lathe, api, infra]
}

/// `projects` projects of `sessions` sessions, in all the statuses.
fn big(projects: usize, sessions: usize) -> Vec<ProjectData> {
    let claude = claude::look();
    (0..projects)
        .map(|p| ProjectData {
            id: format!("p{p}").into(),
            name: format!("project-{p}").into(),
            location: if p % 3 == 0 { Location::Ssh { host: format!("host-{}", p % 5).into() } } else { Location::Local },
            connection: Connection::Connected,
            branch: Some("main".into()),
            pulls_unavailable: None,
        tasks_open: None,
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

pub struct SidebarStory {
    sidebar: Entity<Sidebar>,
    data: Vec<ProjectData>,
    other: AgentLook,
    events: Vec<SharedString>,
    live: bool,
    started: Instant,
    step: u64,
    run: Option<Run>,
    _events: Subscription,
}

impl SidebarStory {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let measuring = std::env::var("GALLERY_SCROLL").is_ok_and(|v| v == "1");
        let other = AgentLook::neutral(cx.theme());
        let env = |name: &str, default: usize| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
        let data = if measuring { big(env("SIDEBAR_PROJECTS", 50), env("SIDEBAR_SESSIONS", 40)) } else { sample(&other) };
        let sidebar = cx.new(|cx| {
            let mut sidebar = Sidebar::new(cx);
            sidebar.set_projects(data.clone(), BASE, cx);
            if measuring && std::env::var("SIDEBAR_OPEN").is_ok_and(|v| v == "1") {
                sidebar.open_all_older(cx);
            }
            sidebar
        });
        let _events = cx.subscribe(&sidebar, |this: &mut Self, _, event: &SidebarEvent, cx| {
            this.events.insert(0, format!("{event:?}").into());
            this.events.truncate(8);
            cx.notify();
        });
        Self {
            sidebar,
            data,
            other,
            events: Vec::new(),
            live: false,
            started: Instant::now(),
            step: 0,
            run: measuring.then(Run::new),
            _events,
        }
    }

    /// One live step: the next session takes the next status and becomes the most recent.
    fn advance(&mut self, cx: &mut Context<Self>) {
        self.step += 1;
        let n = self.step as usize;
        let total: usize = self.data.iter().map(|p| p.sessions.len()).sum();
        let mut at = n % total.max(1);
        let statuses = [
            SessionStatus::Working,
            SessionStatus::NeedsYou(Need::Approval),
            SessionStatus::Finished,
            SessionStatus::Idle,
            SessionStatus::NeedsYou(Need::Question),
            SessionStatus::Failed("lost the connection".into()),
        ];
        let now = BASE + self.started.elapsed().as_secs();
        for project in &mut self.data {
            if at < project.sessions.len() {
                let session = &mut project.sessions[at];
                session.status = statuses[n % statuses.len()].clone();
                session.active_at = now;
                break;
            }
            at -= project.sessions.len();
        }
        // The reconnecting project cycles too.
        if let Some(infra) = self.data.iter_mut().find(|p| p.id == "infra") {
            infra.connection = [Connection::Reconnecting, Connection::Offline, Connection::Connected][n % 3];
        }
        // A new session appears every fourth step.
        if n.is_multiple_of(4) {
            let look = self.other.clone();
            let id = format!("new{n}");
            let title = format!("A new session, step {n}");
            let mut fresh = session(&id, &title, &look, SessionStatus::Working, 0);
            fresh.active_at = now;
            self.data[0].sessions.push(fresh);
        }
        let data = self.data.clone();
        self.sidebar.update(cx, |s, cx| s.set_projects(data, now, cx));
    }
}

impl Render for SidebarStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if let Some(run) = &mut self.run {
            match run.frame() {
                Some(n) => self.sidebar.update(cx, |s, _| s.set_scroll_top(n as f32 * 48.)),
                None => {
                    cx.quit();
                }
            }
            window.request_animation_frame();
        }
        if self.live {
            window.request_animation_frame();
            if self.started.elapsed().as_millis() / STEP.as_millis() > u128::from(self.step) {
                self.advance(cx);
            }
        }
        let live = self.live;
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .child(
                div().flex().items_center().gap(px(8.)).child(
                    Button::new("live")
                        .label(if live { "Stop live" } else { "Live" })
                        .variant(if live { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.live = !this.live;
                            this.started = Instant::now();
                            this.step = 0;
                            cx.notify();
                        })),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(16.))
                    .child(div().w(px(360.)).h_full().rounded(px(12.)).bg(theme.card).child(self.sidebar.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .text_color(theme.muted_foreground)
                            .child(div().text_color(theme.foreground).child("What the sidebar asked for"))
                            .children(self.events.iter().map(|e| div().child(e.clone()))),
                    ),
            );
        match &self.run {
            Some(run) => run.wrap(root.into_any_element()).into_any_element(),
            None => root.into_any_element(),
        }
    }
}
