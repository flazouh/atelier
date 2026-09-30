//! "Open over SSH…": a host, typed or picked from the user's `~/.ssh/config` below the field, and Connect. The
//! folder is chosen next, in the folder picker over the host's own folders. Focus starts on the host; Enter
//! connects, and Escape closes the form. While it connects the form shows each step ("Reaching hp-agent…", "Putting lathe-remote
//! on hp-agent…"); a failure shows ssh's own words and leaves the form open to try again.

use beui::{
    TextInput,
    button::{Button, ButtonSize, ButtonVariant},
    spinner::Spinner,
    theme::{ActiveTheme, popover_shadow, radius},
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Window,
    base::input::Escape,
    component::input::{InputEvent, InputState},
    div, px,
};

/// The chips' names for tests and screenshots, by their place in the list.
const HOST_CHIPS: [&str; 8] = ["ssh-host-0", "ssh-host-1", "ssh-host-2", "ssh-host-3", "ssh-host-4", "ssh-host-5", "ssh-host-6", "ssh-host-7"];

#[derive(Clone, Debug, PartialEq)]
pub enum SshFormEvent {
    Connect { host: String },
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
    host: Entity<InputState>,
    pub phase: Phase,
    _enter: gpui_kit::Subscription,
}

impl EventEmitter<SshFormEvent> for SshForm {}

impl Focusable for SshForm {
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        self.host.focus_handle(cx)
    }
}

impl SshForm {
    pub fn new(hosts: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let host = cx.new(|cx| InputState::new(window, cx).placeholder("user@host, or a host from ~/.ssh/config"));
        if let Some(first) = hosts.first() {
            host.update(cx, |h, cx| h.set_value(first.clone(), window, cx));
        }
        let enter = |this: &mut Self, _: &Entity<InputState>, event: &InputEvent, _: &mut Window, cx: &mut Context<Self>| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.connect(cx);
            }
        };
        let _enter = cx.subscribe_in(&host, window, enter);
        Self { hosts, host, phase: Phase::Idle, _enter }
    }

    /// The config's hosts, once they have been read off the UI thread.
    pub fn set_hosts(&mut self, hosts: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.hosts.is_empty() && self.host.read(cx).value().is_empty()
            && let Some(first) = hosts.first()
        {
            self.host.update(cx, |h, cx| h.set_value(first.clone(), window, cx));
        }
        self.hosts = hosts;
        cx.notify();
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Connecting(_)) {
            return;
        }
        let host = self.host.read(cx).value().trim().to_string();
        if host.is_empty() {
            self.phase = Phase::Failed("Name a host.".into());
            cx.notify();
            return;
        }
        self.phase = Phase::Connecting(format!("Reaching {host}…").into());
        cx.emit(SshFormEvent::Connect { host });
        cx.notify();
    }
}

impl Render for SshForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let field = TextInput::new("ssh-host-field", &self.host).label("Host").surface(theme.popover);
        let this = cx.entity().downgrade();
        // The config's hosts, as chips under the field: a press puts the name in it. They are filled chips that
        // change on hover, and the ones that do not fit the first line wrap under the first chip, not under the label.
        let chips = self.hosts.iter().enumerate().map(|(i, name)| {
            let (host, name) = (self.host.clone(), name.clone());
            Button::new(("ssh-host", i))
                .debug_name(HOST_CHIPS[i.min(HOST_CHIPS.len() - 1)])
                .label(name.clone())
                .variant(ButtonVariant::Secondary)
                .pill(true)
                .on_click(move |_, window, cx| {
                    host.update(cx, |h, cx| {
                        h.set_value(name.clone(), window, cx);
                        h.focus(window, cx);
                    })
                })
        });
        let hosts = (!self.hosts.is_empty()).then(|| {
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .child(div().flex_none().h(px(28.)).flex().items_center().text_size(TextSize::Xs.font_size()).text_color(muted).child("From ~/.ssh/config"))
                .child(div().debug_selector(|| "ssh-hosts".into()).flex().flex_1().min_w_0().flex_wrap().gap(px(6.)).children(chips))
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
            .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Open over SSH"))
            .child(field)
            .children(hosts)
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

#[cfg(test)]
mod tests;
