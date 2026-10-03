use std::time::Instant;

use atelier_ui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, Connection, Need, ProjectData,
    SessionStatus, Sidebar, SidebarEvent, agent_look::AgentLook,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, px,
};

use super::run::Run;
use super::types::{BASE, STEP};
use super::helpers::{big, handoff_targets, sample, session};

pub struct SidebarStory {
    pub(super) sidebar: Entity<Sidebar>,
    data: Vec<ProjectData>,
    pub(super) other: AgentLook,
    events: Vec<SharedString>,
    pub(super) live: bool,
    started: Instant,
    pub(super) step: u64,
    pub(super) run: Option<Run>,
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
            for project in &data {
                sidebar.set_handoff(project.id.clone(), handoff_targets(), cx);
            }
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
