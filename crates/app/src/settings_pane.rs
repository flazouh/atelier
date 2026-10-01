//! The Settings page: what a reader can set, in four sections with a list of them at its left. Appearance holds the
//! theme, light, dark or the system's, and the primary colour: the fill of the main button, and the accent and the
//! selection wash too (see `beui::theme::with_pick`). Agents lists the agents this build can start and the models
//! each offers. Tasks holds the rules that move a task by itself. Keys lists the review's key table, read only for
//! now. A change applies at once, to every window, and is kept in `lathe-settings`. Escape closes the page, and so
//! does the Back button in the title bar.
use beui::{
    ActiveTheme, ColorSelector, Kbd, Segment, Segmented, Swatch,
    sidebar_layout::{BadgeShow, EARLIER_CHOICES, FOLD_CHOICES, SidebarLayout},
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
    /// The sidebar's look changed (what a row shows, how much folds): the new layout, whose mode and filter the shell ignores.
    Sidebar(SidebarLayout),
}

pub struct SettingsPane {
    focus: FocusHandle,
    agents: Vec<AgentRow>,
    mode: Mode,
    /// `"default"` or the name of one of [`PRIMARIES`].
    primary: SharedString,
    /// Which task rules move a task by themselves.
    rules: lathe_tracker::RuleSet,
    section: Section,
    /// The sidebar's look, as this page edits it.
    look: SidebarLayout,
}

/// The sections of the page, in the order the list at its left shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Sidebar,
    Agents,
    Tasks,
    Keys,
}

impl Section {
    pub const ALL: [Section; 5] = [Section::Appearance, Section::Sidebar, Section::Agents, Section::Tasks, Section::Keys];

    pub fn words(self) -> &'static str {
        match self {
            Section::Appearance => "Appearance",
            Section::Sidebar => "Sidebar",
            Section::Agents => "Agents",
            Section::Tasks => "Tasks",
            Section::Keys => "Keys",
        }
    }

    /// One line under the section's name.
    pub fn gist(self) -> &'static str {
        match self {
            Section::Appearance => "The theme, light or dark, and the colour of the main button.",
            Section::Sidebar => "What a session row shows, and how many sessions the sidebar shows before it folds the rest.",
            Section::Agents => "The agents this build can start, and the models each offers.",
            Section::Tasks => "What moves a task by itself. Every move shows in its activity, and you can move it back.",
            Section::Keys => "The keys of the review. They cannot be changed yet.",
        }
    }

    /// The name a test finds the section's entry in the list by.
    pub fn entry(self) -> &'static str {
        match self {
            Section::Appearance => "section-appearance",
            Section::Sidebar => "section-sidebar",
            Section::Agents => "section-agents",
            Section::Tasks => "section-tasks",
            Section::Keys => "section-keys",
        }
    }
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
        let rules = lathe_tracker::RuleSet::from_disabled(saved.task_rules_off.iter().map(String::as_str));
        Self {
            focus: cx.focus_handle(),
            agents,
            mode,
            primary: primary.into(),
            rules,
            section: Section::Appearance,
            look: crate::sidebar_layout::from_settings(saved),
        }
    }

    fn choose_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        mode.apply(cx);
        save(cx, move |s| s.mode = Some(mode.key().into()));
        cx.notify();
    }

    pub(crate) fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }
    /// Changes the sidebar's look: kept in the settings and applied at once.
    pub(crate) fn change_look(&mut self, change: impl FnOnce(&mut SidebarLayout), cx: &mut Context<Self>) {
        change(&mut self.look);
        let saved = crate::sidebar_layout::saved_look(&self.look);
        save(cx, move |s| s.sidebar_layout = saved);
        cx.emit(SettingsEvent::Sidebar(self.look));
        cx.notify();
    }

    fn set_rule(&mut self, rule: lathe_tracker::Rule, on: bool, cx: &mut Context<Self>) {
        self.rules.set(rule, on);
        let off: Vec<String> = self.rules.disabled().into_iter().map(String::from).collect();
        save(cx, move |s| s.task_rules_off = off);
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

/// The name a test finds a rule's switch by.
pub(crate) fn rule_switch(rule: lathe_tracker::Rule) -> &'static str {
    match rule {
        lathe_tracker::Rule::SessionStartMovesToInProgress => "rule-session-start",
        lathe_tracker::Rule::AgentFinishMovesToInReview => "rule-agent-finish",
        lathe_tracker::Rule::MergeMovesToDone => "rule-merge",
        lathe_tracker::Rule::SessionResumeMovesToInProgress => "rule-session-resume",
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

        let appearance = div()
            .flex()
            .flex_col()
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
            );
        let sidebar = {
            let look = self.look;
            let (badge_pane, time_pane, icon_pane, fold_pane, earlier_pane) = (this.clone(), this.clone(), this.clone(), this.clone(), this.clone());
            let numbers = |id: &'static str, choices: &'static [usize], current: usize, names: &'static [&'static str], pane: gpui_kit::WeakEntity<SettingsPane>, set: fn(&mut SidebarLayout, usize)| {
                Segmented::new(id, choices.iter().zip(names).map(|(n, name)| Segment::new(n.to_string()).debug_name(name)), choices.iter().position(|c| *c == current).unwrap_or(0))
                    .on_change(move |i, _, cx| {
                        pane.update(cx, |p, cx| p.change_look(|l| set(l, choices[i]), cx)).ok();
                    })
                    .into_any_element()
            };
            div()
                .flex()
                .flex_col()
                .child(row(
                    "Project badge on rows",
                    Segmented::new(
                        "sidebar-badge",
                        BadgeShow::ALL.iter().map(|b| Segment::new(b.words()).debug_name(match b {
                            BadgeShow::Auto => "badge-auto",
                            BadgeShow::Always => "badge-always",
                            BadgeShow::Never => "badge-never",
                        })),
                        BadgeShow::ALL.iter().position(|b| *b == look.project_badge).unwrap_or(0),
                    )
                    .on_change(move |i, _, cx| {
                        badge_pane.update(cx, |p, cx| p.change_look(|l| l.project_badge = BadgeShow::ALL[i], cx)).ok();
                    })
                    .into_any_element(),
                ))
                .child(row(
                    "Time on rows",
                    beui::Switch::new("sidebar-time", look.show_time)
                        .debug_name("sidebar-time")
                        .on_change(move |on, _, cx| {
                            time_pane.update(cx, |p, cx| p.change_look(|l| l.show_time = on, cx)).ok();
                        })
                        .into_any_element(),
                ))
                .child(row(
                    "Agent icon on rows",
                    beui::Switch::new("sidebar-icon", look.show_agent_icon)
                        .debug_name("sidebar-icon")
                        .on_change(move |on, _, cx| {
                            icon_pane.update(cx, |p, cx| p.change_look(|l| l.show_agent_icon = on, cx)).ok();
                        })
                        .into_any_element(),
                ))
                .child(row("Sessions shown for each project", numbers("sidebar-fold", &FOLD_CHOICES, look.fold_after, &["fold-3", "fold-5", "fold-8", "fold-12"], fold_pane, |l, n| l.fold_after = n)))
                .child(row("Earlier sessions shown in Priority", numbers("sidebar-earlier", &EARLIER_CHOICES, look.earlier_shown, &["earlier-5", "earlier-8", "earlier-12", "earlier-20"], earlier_pane, |l, n| l.earlier_shown = n)))
        };
        let tasks = div().flex().flex_col().children(lathe_tracker::Rule::ALL.into_iter().map(|rule| {
            let pane = this.clone();
            row(
                rule.words(),
                beui::Switch::new(rule.id(), self.rules.is_on(rule))
                    .debug_name(rule_switch(rule))
                    .on_change(move |on, _, cx| {
                        pane.update(cx, |p, cx| p.set_rule(rule, on, cx)).ok();
                    })
                    .into_any_element(),
            )
        }));
        let agents = div()
            .flex()
            .flex_col()
            .children(agent_rows)
            .when(self.agents.is_empty(), |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("No agent is available.")));
        let keys_list = div().flex().flex_col().children(key_rows);
        let body = match self.section {
            Section::Appearance => appearance.into_any_element(),
            Section::Sidebar => sidebar.into_any_element(),
            Section::Agents => agents.into_any_element(),
            Section::Tasks => tasks.into_any_element(),
            Section::Keys => keys_list.into_any_element(),
        };
        // The list of sections, at the left: the front one on a card.
        let nav = div()
            .debug_selector(|| "settings-sections".into())
            .flex_none()
            .w(px(200.))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .pt(px(48.))
            .child(div().px(px(8.)).pb(px(12.)).text_size(TextSize::Lg.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child("Settings"))
            .children(Section::ALL.into_iter().map(|section| {
                let pane = this.clone();
                let front = section == self.section;
                div()
                    .id(section.entry())
                    .debug_selector(move || section.entry().to_string())
                    .flex()
                    .items_center()
                    .h(px(32.))
                    .px(px(8.))
                    .rounded(beui::theme::radius::MD)
                    .cursor_pointer()
                    .text_size(TextSize::Sm.font_size())
                    .text_color(if front { theme.foreground } else { muted })
                    .when(front, |d| d.bg(theme.card_strong).font_weight(FontWeight::MEDIUM))
                    .hover(|s| s.bg(theme.card_strong.opacity(0.6)))
                    .on_click(move |_, _, cx| {
                        pane.update(cx, |p, cx| p.show(section, cx)).ok();
                    })
                    .child(section.words())
            }));
        div()
            .id("settings-pane")
            .key_context("SettingsPane")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(SettingsEvent::Close);
                }
            }))
            .flex()
            .size_full()
            .bg(theme.background)
            .child(nav)
            .child(
                div()
                    .id("settings-scroll")
                    .debug_selector(|| "settings-scroll".into())
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(640.))
                            .px(px(24.))
                            .py(px(48.))
                            .flex()
                            .flex_col()
                            .child(div().text_size(TextSize::Xl.font_size()).font_weight(FontWeight::MEDIUM).text_color(theme.foreground).child(self.section.words()))
                            .child(div().pt(px(4.)).pb(px(16.)).text_size(TextSize::Sm.font_size()).text_color(muted).child(self.section.gist()))
                            .child(body),
                    ),
            )
    }
}

#[cfg(test)]
mod tests;
