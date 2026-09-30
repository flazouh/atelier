//! The Settings pane: what a reader can set, in three sections. Appearance holds the theme, light, dark or the
//! system's, and the primary colour: the fill of the main button, and the accent and the selection wash too (see
//! `beui::theme::with_pick`). Keys lists the review's key table, read only for now. Agents lists the agents this
//! build can start and the models each offers. A change applies at once, to every window, and is kept in
//! `lathe-settings`. Escape closes the pane.
use beui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, ColorSelector, IconName, Kbd, Segment, Segmented, Swatch,
    keys,
    theme::{Appearance, can_be_primary, follow_system, set_appearance, set_pick},
    theme_picker::theme_picker,
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, EventEmitter, FocusHandle, Focusable, FontWeight, Hsla, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Rgba, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

/// Light, dark, or whatever the system is set to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
    System,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Light, Mode::Dark, Mode::System];

    pub fn word(self) -> &'static str {
        match self {
            Mode::Light => "Light",
            Mode::Dark => "Dark",
            Mode::System => "System",
        }
    }

    /// As settings keep it.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
            Mode::System => "system",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }

    /// Puts the mode in force: the current family's light or dark theme, or the system's.
    pub fn apply(self, cx: &mut gpui_kit::App) {
        match self {
            Mode::Light => set_appearance(Appearance::Light, cx),
            Mode::Dark => set_appearance(Appearance::Dark, cx),
            Mode::System => follow_system(cx),
        }
    }
}

/// The primary colours offered, besides the default (the theme's ink): name, colour as bytes, words. They are
/// the reader's data, not the interface's colours.
pub const PRIMARIES: [(&str, [u8; 3], &str); 8] = [
    ("blue", [2, 133, 247], "Blue"),
    ("purple", [146, 112, 232], "Purple"),
    ("pink", [230, 106, 164], "Pink"),
    ("red", [229, 86, 86], "Red"),
    ("orange", [237, 145, 65], "Orange"),
    ("amber", [229, 182, 60], "Amber"),
    ("green", [101, 166, 90], "Green"),
    ("teal", [22, 157, 131], "Teal"),
];

/// The colour a byte triple names.
pub fn colour(bytes: [u8; 3]) -> Hsla {
    Rgba { r: bytes[0] as f32 / 255., g: bytes[1] as f32 / 255., b: bytes[2] as f32 / 255., a: 1. }.into()
}

/// An agent the build can start, as the pane lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRow {
    pub name: SharedString,
    pub models: Vec<SharedString>,
}

pub enum SettingsEvent {
    Close,
}

pub struct SettingsPane {
    focus: FocusHandle,
    agents: Vec<AgentRow>,
    mode: Mode,
    /// `"default"` or the name of one of [`PRIMARIES`].
    primary: SharedString,
}

impl EventEmitter<SettingsEvent> for SettingsPane {}

impl Focusable for SettingsPane {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsPane {
    pub fn new(saved: &lathe_settings::Settings, agents: Vec<AgentRow>, cx: &mut Context<Self>) -> Self {
        let mode = saved.mode.as_deref().and_then(Mode::from_key).unwrap_or(Mode::System);
        let primary = saved
            .primary
            .and_then(|bytes| PRIMARIES.iter().find(|(_, b, _)| *b == bytes))
            .map_or("default", |(name, _, _)| *name);
        Self { focus: cx.focus_handle(), agents, mode, primary: primary.into() }
    }

    fn choose_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        mode.apply(cx);
        save(cx, move |s| s.mode = Some(mode.key().into()));
        cx.notify();
    }

    fn choose_primary(&mut self, name: &SharedString, cx: &mut Context<Self>) {
        let bytes = PRIMARIES.iter().find(|(n, _, _)| *n == name.as_ref()).map(|(_, b, _)| *b);
        self.primary = name.clone();
        set_pick(bytes.map(colour), cx);
        save(cx, move |s| s.primary = bytes);
        cx.notify();
    }
}

/// Keeps a change, off the UI thread.
fn save(cx: &mut gpui_kit::App, change: impl FnOnce(&mut lathe_settings::Settings) + Send + 'static) {
    if let Some(path) = lathe_settings::path() {
        cx.background_spawn(async move {
            if let Err(error) = lathe_settings::update(&path, change) {
                eprintln!("could not save the settings: {error}");
            }
        })
        .detach();
    }
}

impl Render for SettingsPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let this = cx.entity().downgrade();

        let heading = |words: &'static str| div().pt(px(28.)).pb(px(6.)).text_size(TextSize::Sm.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(words);
        let row = |label: &'static str, control: gpui_kit::AnyElement| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.))
                .min_h(px(44.))
                .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child(label))
                .child(control)
        };

        let modes = {
            let this = this.clone();
            let segments = Mode::ALL.into_iter().map(|mode| {
                Segment::new(mode.word()).debug_name(match mode {
                    Mode::Light => "mode-light",
                    Mode::Dark => "mode-dark",
                    Mode::System => "mode-system",
                })
            });
            Segmented::new("mode", segments, Mode::ALL.iter().position(|m| *m == self.mode).unwrap_or(0)).on_change(move |i, _, cx| {
                this.update(cx, |pane, cx| pane.choose_mode(Mode::ALL[i], cx)).ok();
            })
        };
        let swatches = std::iter::once(Swatch::new("default", theme.foreground, "Default: the theme's ink")).chain(PRIMARIES.iter().map(|(name, bytes, words)| {
            let color = colour(*bytes);
            Swatch::new(*name, color, *words).disabled(!can_be_primary(&theme, color))
        }));
        let picker = ColorSelector::new("primary", swatches).value(Some(self.primary.clone())).on_change({
            let this = this.clone();
            move |value, _, cx| {
                let value = value.clone();
                this.update(cx, |pane, cx| pane.choose_primary(&value, cx)).ok();
            }
        });

        let profile = keys::profile(cx);
        let key_rows = keys::KEYBOARD.iter().map(|(command, word, gist)| {
            let chord = keys::chord_for(profile, *command);
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .min_h(px(36.))
                .child(div().w(px(160.)).flex_none().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child(*word))
                .child(div().flex_1().min_w_0().text_size(TextSize::Xs.font_size()).text_color(muted).child(*gist))
                .children(chord.map(|c| Kbd::new(keys::cap(c))))
        });

        let agent_rows = self.agents.iter().enumerate().map(|(i, agent)| {
            div()
                .debug_selector(move || format!("agent-row-{i}"))
                .flex()
                .gap(px(12.))
                .py(px(8.))
                .child(div().w(px(160.)).flex_none().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child(agent.name.clone()))
                .child(div().flex_1().min_w_0().text_size(TextSize::Xs.font_size()).text_color(muted).child(if agent.models.is_empty() {
                    SharedString::from("No model to pick")
                } else {
                    SharedString::from(agent.models.iter().map(|m| m.as_ref()).collect::<Vec<_>>().join(", "))
                }))
        });

        let close = this.clone();
        div()
            .id("settings-pane")
            .key_context("SettingsPane")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(SettingsEvent::Close);
                }
            }))
            .relative()
            .size_full()
            .bg(theme.background)
            .child(
                div()
                    .id("settings-scroll")
                    .debug_selector(|| "settings-scroll".into())
                    .size_full()
                    .overflow_y_scroll()
                    .child(
                div()
                    .mx_auto()
                    .w_full()
                    .max_w(px(640.))
                    .px(px(24.))
                    .py(px(48.))
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child("Settings"))
                    .child(heading("Appearance"))
                    .child(row("Theme", div().w(px(220.)).child(theme_picker("settings-theme", &theme, |picked, cx| {
                        let name = picked.name.to_string();
                        save(cx, move |s| s.theme = Some(name));
                    })).into_any_element()))
                    .child(row("Mode", modes.into_any_element()))
                    .child(
                        div()
                            .pt(px(12.))
                            .flex()
                            .flex_col()
                            .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child("Primary colour"))
                            .child(div().pb(px(12.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("The main button's fill, and the highlight and the selection. A colour that no text reads on is not offered."))
                            .child(picker),
                    )
                    .child(heading("Keys"))
                    .child(div().pb(px(6.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("The keys of the review. They cannot be changed yet."))
                    .children(key_rows)
                    .child(heading("Agents"))
                    .child(div().flex().flex_col().children(agent_rows))
                    .when(self.agents.is_empty(), |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("No agent is available."))),
                    ),
            )
            .child(
                div().absolute().top(px(16.)).right(px(16.)).child(
                    Button::new("settings-close")
                        .debug_name("settings-close")
                        .icon(IconName::Close)
                        .cap("Esc")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, _, cx| {
                            close.update(cx, |_, cx| cx.emit(SettingsEvent::Close)).ok();
                        }),
                ),
            )
    }
}

#[cfg(test)]
mod tests;
