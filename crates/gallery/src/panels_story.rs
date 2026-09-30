//! The "Panels" story: six agent panels of two projects, side by side and in a single view, grouped by
//! project and not. `PANELS=12` opens twelve. `GALLERY_SCROLL=1` scrolls the strip sideways and prints the
//! frame numbers; with `GALLERY_SWITCH=1` it also switches the layout every 30th frame and counts those
//! frames apart.

use beui::{
    ActiveTheme, AgentPanels, Button, ButtonSize, ButtonVariant, Location, Need, PanelData, PanelLayout, PanelsEvent, PanelsState, ProjectLabel,
    SessionStatus,
    agent_look::AgentLook,
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use lathe_agents::claude;

use crate::sidebar_story::run::Run;

const TITLES: [&str; 6] = [
    "Add the sidebar and the agent panels",
    "Which endpoints still return 500?",
    "Port GitQuiet's Court sorting",
    "Migrate the sessions table",
    "Why does the diff miss 120Hz?",
    "Write the OpenAPI notes",
];

fn projects() -> [ProjectLabel; 2] {
    [
        ProjectLabel { id: "lathe".into(), name: "lathe".into(), location: Location::Local },
        ProjectLabel { id: "api".into(), name: "api-server".into(), location: Location::Ssh { host: "hp-agent".into() } },
    ]
}

fn status_of(n: usize) -> SessionStatus {
    match n % 6 {
        0 => SessionStatus::Working,
        1 => SessionStatus::NeedsYou(Need::Approval),
        2 => SessionStatus::Finished,
        3 => SessionStatus::Idle,
        4 => SessionStatus::NeedsYou(Need::Question),
        _ => SessionStatus::Failed("lost the connection".into()),
    }
}

/// What a panel holds in the story: its title, its status in words, and some lines to scroll past.
fn body(title: SharedString, status: SessionStatus) -> impl Fn(&mut Window, &mut gpui_kit::App) -> AnyElement {
    move |_, cx| {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(14.))
            .text_color(theme.foreground)
            .child(div().child(title.clone()))
            .child(div().text_color(theme.muted_foreground).child(status.words()))
            .children((0..40).map(|i| {
                div().text_color(theme.muted_foreground).child(format!("Line {i} of what the agent said in this panel, kept as it was."))
            }))
            .into_any_element()
    }
}

fn panel(n: usize, look: &AgentLook, cx: &mut gpui_kit::App) -> PanelData {
    let project = projects()[n % 2].clone();
    let title: SharedString = if n < TITLES.len() { TITLES[n].into() } else { format!("Session number {n}").into() };
    let status = status_of(n);
    PanelData {
        id: format!("s{n}").into(),
        project,
        title: title.clone(),
        look: look.clone(),
        status: status.clone(),
        content: beui::panel_types::content_from(body(title, status), cx),
    }
}

pub struct PanelsStory {
    panels: Entity<AgentPanels>,
    data: Vec<PanelData>,
    next: usize,
    look: AgentLook,
    run: Option<Run>,
    switching: bool,
    _events: Subscription,
}

impl PanelsStory {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let look = claude::look();
        let count = std::env::var("PANELS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
        let data: Vec<PanelData> = (0..count).map(|n| panel(n, &look, cx)).collect();
        let order: Vec<SharedString> = projects().iter().map(|p| p.id.clone()).collect();
        let panels = cx.new(|cx| {
            let mut panels = AgentPanels::new(cx);
            panels.set_panels(data.clone(), order.clone(), cx);
            if std::env::var("PANELS_LAYOUT").is_ok_and(|v| v == "single") {
                panels.restore(PanelsState { layout: PanelLayout::Single, grouped: true, widths: Vec::new() }, cx);
            }
            panels
        });
        let _events = cx.subscribe(&panels, |this: &mut Self, panels, event: &PanelsEvent, cx| {
            if let PanelsEvent::Closed(id) = event {
                this.data.retain(|p| p.id != *id);
                let (data, order) = (this.data.clone(), projects().iter().map(|p| p.id.clone()).collect());
                panels.update(cx, |p, cx| p.set_panels(data, order, cx));
            }
        });
        let measuring = std::env::var("GALLERY_SCROLL").is_ok_and(|v| v == "1");
        Self {
            panels,
            data,
            next: count,
            look,
            run: measuring.then(Run::new),
            switching: std::env::var("GALLERY_SWITCH").is_ok_and(|v| v == "1"),
            _events,
        }
    }

    fn open_another(&mut self, cx: &mut Context<Self>) {
        let next = panel(self.next, &self.look, cx);
        self.data.push(next);
        self.next += 1;
        let (data, order) = (self.data.clone(), projects().iter().map(|p| p.id.clone()).collect());
        self.panels.update(cx, |p, cx| p.set_panels(data, order, cx));
    }
}

impl Render for PanelsStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(run) = &mut self.run {
            match run.frame() {
                Some(n) => {
                    let switch = self.switching && n % 30 == 29;
                    self.panels.update(cx, |p, cx| {
                        if switch {
                            p.toggle_layout(cx);
                        }
                        p.scroll_to((n % 200) as f32 * 40., cx);
                    });
                    if switch {
                        run.switched();
                    }
                }
                None => {
                    cx.quit();
                }
            }
            window.request_animation_frame();
        }
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(16.))
            .child(
                div().flex().items_center().child(
                    Button::new("open-another")
                        .label("Open another panel")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| this.open_another(cx))),
                ),
            )
            .child(div().flex_1().min_h_0().child(self.panels.clone()));
        match &self.run {
            Some(run) => run.wrap(root.into_any_element()).into_any_element(),
            None => root.into_any_element(),
        }
    }
}
