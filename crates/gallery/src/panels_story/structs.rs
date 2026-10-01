use atelier_ui::{
    AgentPanels, Button, ButtonSize, ButtonVariant, PanelData, PanelLayout, PanelsEvent,
    PanelsState, agent_look::AgentLook,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, px,
};
use atelier_agents::claude;

use crate::sidebar_story::run::Run;
use super::helpers::{panel, projects};

pub struct PanelsStory {
    pub(super) panels: Entity<AgentPanels>,
    pub(super) data: Vec<PanelData>,
    pub(super) next: usize,
    pub(super) look: AgentLook,
    pub(super) run: Option<Run>,
    pub(super) switching: bool,
    pub(super) _events: Subscription,
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

    pub(super) fn open_another(&mut self, cx: &mut Context<Self>) {
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
