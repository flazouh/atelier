//! "Open over SSH…": a host from the user's `~/.ssh/config` (or one typed), a folder on it, and
//! Connect. While it connects the form shows each step ("Reaching hp-agent…", "Putting lathe-remote
//! on hp-agent…"); a failure shows ssh's own words and leaves the form open to try again.

use beui::{
    button::{Button, ButtonSize, ButtonVariant},
    select::Select,
    spinner::Spinner,
    theme::{ActiveTheme, popover_shadow, radius},
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Window,
    base::input::Escape,
    component::input::{Input, InputEvent, InputState},
    div, px,
};

#[derive(Clone, Debug, PartialEq)]
pub enum SshFormEvent {
    Connect { host: String, path: String },
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// Connecting: the step it is on.
    Connecting(SharedString),
    Failed(SharedString),
}

pub struct SshForm {
    hosts: Vec<String>,
    picked: Option<usize>,
    host: Entity<InputState>,
    path: Entity<InputState>,
    pub phase: Phase,
    _enter: [gpui_kit::Subscription; 2],
}

impl EventEmitter<SshFormEvent> for SshForm {}

impl Focusable for SshForm {
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        self.path.focus_handle(cx)
    }
}

impl SshForm {
    pub fn new(hosts: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let host = cx.new(|cx| InputState::new(window, cx).placeholder("user@host, or a host from ~/.ssh/config"));
        let path = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("~/code/project");
            state.set_value("~/", window, cx);
            state
        });
        if let Some(first) = hosts.first() {
            host.update(cx, |h, cx| h.set_value(first.clone(), window, cx));
        }
        let enter = |this: &mut Self, _: &Entity<InputState>, event: &InputEvent, _: &mut Window, cx: &mut Context<Self>| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.connect(cx);
            }
        };
        let _enter = [cx.subscribe_in(&host, window, enter), cx.subscribe_in(&path, window, enter)];
        Self { picked: (!hosts.is_empty()).then_some(0), hosts, host, path, phase: Phase::Idle, _enter }
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Connecting(_)) {
            return;
        }
        let host = self.host.read(cx).value().trim().to_string();
        let path = self.path.read(cx).value().trim().to_string();
        if host.is_empty() || path.is_empty() {
            self.phase = Phase::Failed("Name a host and a folder.".into());
            cx.notify();
            return;
        }
        self.phase = Phase::Connecting(format!("Reaching {host}…").into());
        cx.emit(SshFormEvent::Connect { host, path });
        cx.notify();
    }
}

impl Render for SshForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let field = |label: &'static str, input: &Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child(label))
                .child(div().rounded(radius::LG).bg(theme.card_strong).child(Input::new(input).appearance(false).px(px(10.)).text_size(TextSize::Sm.font_size())))
        };
        let this = cx.entity().downgrade();
        let hosts = (!self.hosts.is_empty()).then(|| {
            let (names, picked) = (self.hosts.clone(), self.picked);
            let (host, pick) = (self.host.clone(), this.clone());
            Select::new("ssh-hosts", names.clone()).selected(picked).compact(true).on_change(move |i, window, cx| {
                host.update(cx, |h, cx| h.set_value(names[i].clone(), window, cx));
                pick.update(cx, |f, cx| {
                    f.picked = Some(i);
                    cx.notify();
                })
                .ok();
            })
        });
        let status = match &self.phase {
            Phase::Idle => None,
            Phase::Connecting(step) => Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(Spinner::new("ssh-connecting").size(px(12.)).color(muted))
                    .child(step.clone())
                    .into_any_element(),
            ),
            Phase::Failed(why) => Some(div().text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(why.clone()).into_any_element()),
        };
        let connecting = matches!(self.phase, Phase::Connecting(_));
        let (go, cancel) = (this.clone(), this);
        div()
            .id("ssh-form")
            .key_context("SshForm")
            .occlude()
            .on_action(cx.listener(|_, _: &Escape, _, cx| cx.emit(SshFormEvent::Cancel)))
            .flex()
            .flex_col()
            .gap(px(12.))
            .w(px(440.))
            .p(px(16.))
            .rounded(radius::XL)
            .bg(theme.popover)
            .shadow(popover_shadow(&theme))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Open over SSH"))
                    .children(hosts),
            )
            .child(field("Host", &self.host))
            .child(field("Folder on the host", &self.path))
            .children(status)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(Button::new("ssh-cancel").label("Cancel").variant(ButtonVariant::Ghost).cap("Esc").on_click(move |_, _, cx| {
                        cancel.update(cx, |_, cx| cx.emit(SshFormEvent::Cancel)).ok();
                    }))
                    .child(
                        Button::new("ssh-connect")
                            .label("Connect")
                            .size(ButtonSize::Md)
                            .variant(ButtonVariant::Primary)
                            .cap("↵")
                            .disabled(connecting)
                            .on_click(move |_, _, cx| {
                                go.update(cx, |f, cx| f.connect(cx)).ok();
                            }),
                    ),
            )
    }
}
