use atelier_ui::{
    ActiveTheme, Location, Need, PanelData, ProjectLabel, SessionStatus, agent_look::AgentLook,
};
use gpui_kit::{AnyElement, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use super::types::TITLES;

pub(super) fn projects() -> [ProjectLabel; 2] {
    [
        ProjectLabel { id: "atelier".into(), name: "atelier".into(), location: Location::Local },
        ProjectLabel { id: "api".into(), name: "api-server".into(), location: Location::Ssh { host: "hp-agent".into() } },
    ]
}

pub(super) fn status_of(n: usize) -> SessionStatus {
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
pub(super) fn body(title: SharedString, status: SessionStatus) -> impl Fn(&mut Window, &mut gpui_kit::App) -> AnyElement {
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

pub(super) fn panel(n: usize, look: &AgentLook, cx: &mut gpui_kit::App) -> PanelData {
    let project = projects()[n % 2].clone();
    let title: SharedString = if n < TITLES.len() { TITLES[n].into() } else { format!("Session number {n}").into() };
    let status = status_of(n);
    PanelData {
        id: format!("s{n}").into(),
        project,
        title: title.clone(),
        look: look.clone(),
        status: status.clone(),
        content: atelier_ui::panel_types::content_from(body(title, status), cx),
    }
}
