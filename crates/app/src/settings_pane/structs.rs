use atelier_ui::{
    ActiveTheme,
    ColorSelector,
    Kbd,
    RangeSlider,
    Segment,
    Segmented,
    Swatch,
    keys,
    sidebar_layout::{BadgeShow, EARLIER_CHOICES, FOLD_CHOICES, SidebarLayout},
    theme::{can_be_primary, set_pick},
    theme_picker::theme_picker,
    typography::TextSize,
};
use gpui_kit::{
    Context, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder,
};
use atelier_ui::scale::px;

use crate::tool_density::ToolDensity;
use crate::agent_session::dictation;
use super::types::{DICTATION_KEYS, Mode, PRIMARIES, Section, SettingsEvent};
use super::helpers::{colour, font_size_words, rule_switch, save};

/// An agent the build can start, as the pane lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRow {
    pub name: SharedString,
    pub models: Vec<SharedString>,
}

pub struct SettingsPane {
    pub(super) focus: FocusHandle,
    pub(super) agents: Vec<AgentRow>,
    pub(super) mode: Mode,
    /// `"default"` or the name of one of [`PRIMARIES`].
    pub(super) primary: SharedString,
    /// Which task rules move a task by themselves.
    pub(super) rules: atelier_tracker::RuleSet,
    pub(super) section: Section,
    /// The sidebar's look, as this page edits it.
    pub(super) look: SidebarLayout,
    /// The font size a drag on its slider is at. The zoom waits for the release: zooming as it moves rescales the
    /// slider under the pointer.
    pub(super) zoom_preview: Option<f32>,
    /// The microphones found when the Dictation section was last shown, its rows as the composer's menu has them.
    pub(super) mics: Vec<atelier_ui::VoiceDevice>,
}

impl EventEmitter<SettingsEvent> for SettingsPane {}

impl Focusable for SettingsPane {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsPane {
    pub fn new(saved: &atelier_settings::Settings, agents: Vec<AgentRow>, cx: &mut Context<Self>) -> Self {
        let mode = saved.mode.as_deref().and_then(Mode::from_key).unwrap_or(Mode::System);
        let primary = saved
            .primary
            .and_then(|bytes| PRIMARIES.iter().find(|(_, b, _)| *b == bytes))
            .map_or("default", |(name, _, _)| *name);
        let rules = atelier_tracker::RuleSet::from_disabled(saved.task_rules_off.iter().map(String::as_str));
        Self {
            focus: cx.focus_handle(),
            agents,
            mode,
            primary: primary.into(),
            rules,
            section: Section::Appearance,
            look: crate::sidebar_layout::from_settings(saved),
            zoom_preview: None,
            mics: Vec::new(),
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
        if section == Section::Dictation {
            self.mics = dictation::device_rows(&atelier_voice::devices(), None).0;
        }
        cx.notify();
    }
    pub(crate) fn choose_dictation(&mut self, change: impl FnOnce(&mut dictation::Prefs), cx: &mut Context<Self>) {
        dictation::choose(cx, change);
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

    fn set_rule(&mut self, rule: atelier_tracker::Rule, on: bool, cx: &mut Context<Self>) {
        self.rules.set(rule, on);
        let off: Vec<String> = self.rules.disabled().into_iter().map(String::from).collect();
        save(cx, move |s| s.task_rules_off = off);
        cx.notify();
    }
    fn set_run_picked_skills(&mut self, on: bool, cx: &mut Context<Self>) {
        cx.set_global(crate::agent_session::RunPickedSkills(on));
        save(cx, move |s| s.run_picked_skills = Some(on));
        cx.notify();
    }
    fn choose_tool_density(&mut self, density: ToolDensity, cx: &mut Context<Self>) {
        cx.set_global(density);
        save(cx, move |s| s.tool_density = Some(density.key().into()));
        cx.notify();
    }
    pub(crate) fn preview_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom_preview = Some(zoom);
        cx.notify();
    }
    pub(crate) fn choose_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom_preview = None;
        cx.emit(SettingsEvent::Zoom(zoom));
        cx.notify();
    }
    pub(super) fn shown_zoom(&self) -> f32 {
        self.zoom_preview.unwrap_or_else(atelier_ui::scale::zoom)
    }
    fn choose_primary(&mut self, name: &SharedString, cx: &mut Context<Self>) {
        let bytes = PRIMARIES.iter().find(|(n, _, _)| *n == name.as_ref()).map(|(_, b, _)| *b);
        self.primary = name.clone();
        set_pick(bytes.map(colour), cx);
        save(cx, move |s| s.primary = bytes);
        cx.notify();
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
            .child(row("Interface font size", {
                let (this, ends) = (this.clone(), this.clone());
                let zoom = self.shown_zoom();
                div()
                    .debug_selector(|| "font-size".into())
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div().w(px(140.)).child(
                            RangeSlider::new("font-size-slider", zoom)
                                .compact(true)
                                .range(atelier_ui::scale::MIN, atelier_ui::scale::MAX)
                                .step(atelier_ui::scale::STEP)
                                .on_change(move |zoom, _, cx| {
                                    this.update(cx, |pane, cx| pane.preview_zoom(zoom, cx)).ok();
                                })
                                .on_end(move |zoom, _, cx| {
                                    ends.update(cx, |pane, cx| pane.choose_zoom(zoom, cx)).ok();
                                }),
                        ),
                    )
                    .child(div().w(px(40.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(font_size_words(zoom)))
                    .into_any_element()
            }))
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
                    atelier_ui::Switch::new("sidebar-time", look.show_time)
                        .debug_name("sidebar-time")
                        .on_change(move |on, _, cx| {
                            time_pane.update(cx, |p, cx| p.change_look(|l| l.show_time = on, cx)).ok();
                        })
                        .into_any_element(),
                ))
                .child(row(
                    "Agent icon on rows",
                    atelier_ui::Switch::new("sidebar-icon", look.show_agent_icon)
                        .debug_name("sidebar-icon")
                        .on_change(move |on, _, cx| {
                            icon_pane.update(cx, |p, cx| p.change_look(|l| l.show_agent_icon = on, cx)).ok();
                        })
                        .into_any_element(),
                ))
                .child(row("Sessions shown for each project", numbers("sidebar-fold", &FOLD_CHOICES, look.fold_after, &["fold-3", "fold-5", "fold-8", "fold-12"], fold_pane, |l, n| l.fold_after = n)))
                .child(row("Earlier sessions shown in Priority", numbers("sidebar-earlier", &EARLIER_CHOICES, look.earlier_shown, &["earlier-5", "earlier-8", "earlier-12", "earlier-20"], earlier_pane, |l, n| l.earlier_shown = n)))
        };
        let tasks = div().flex().flex_col().children(atelier_tracker::Rule::ALL.into_iter().map(|rule| {
            let pane = this.clone();
            row(
                rule.words(),
                atelier_ui::Switch::new(rule.id(), self.rules.is_on(rule))
                    .debug_name(rule_switch(rule))
                    .on_change(move |on, _, cx| {
                        pane.update(cx, |p, cx| p.set_rule(rule, on, cx)).ok();
                    })
                    .into_any_element(),
            )
        }));
        let skills_pane = this.clone();
        let density_pane = this.clone();
        let density = {
            let segments = ToolDensity::ALL.into_iter().map(|d| Segment::new(d.word()).debug_name(match d {
                ToolDensity::Grouped => "tool-density-grouped",
                ToolDensity::Lines => "tool-density-lines",
                ToolDensity::Detailed => "tool-density-detailed",
            }));
            let current = ToolDensity::ALL.iter().position(|d| *d == crate::tool_density::tool_density(cx)).unwrap_or(0);
            Segmented::new("tool-density", segments, current).on_change(move |i, _, cx| {
                density_pane.update(cx, |p, cx| p.choose_tool_density(ToolDensity::ALL[i], cx)).ok();
            })
        };
        let agents = div()
            .flex()
            .flex_col()
            .child(row("Tool calls", density.into_any_element()))
            .child(row(
                "Run a skill when you pick it",
                atelier_ui::Switch::new("skills-run-when-picked", crate::agent_session::runs_picked_skills(cx))
                    .debug_name("skills-run-when-picked")
                    .on_change(move |on, _, cx| {
                        skills_pane.update(cx, |p, cx| p.set_run_picked_skills(on, cx)).ok();
                    })
                    .into_any_element(),
            ))
            .children(agent_rows)
            .when(self.agents.is_empty(), |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("No agent is available.")));
        let speech = dictation::prefs(cx);
        let dictation_pane = {
            let (key_pane, mic_pane, hold_pane) = (this.clone(), this.clone(), this.clone());
            let keys = Segmented::new(
                "dictation-key",
                DICTATION_KEYS.into_iter().map(|k| Segment::new(k.map_or("Off", |k| k.words())).debug_name(k.map_or("dictation-key-off", |k| match k {
                    atelier_voice::hotkey::Key::Fn => "dictation-key-fn",
                    atelier_voice::hotkey::Key::RightOption => "dictation-key-right-option",
                    atelier_voice::hotkey::Key::LeftOption => "dictation-key-left-option",
                }))),
                DICTATION_KEYS.iter().position(|k| *k == speech.key).unwrap_or(0),
            )
            .on_change(move |i, _, cx| {
                key_pane.update(cx, |p, cx| p.choose_dictation(|d| d.key = DICTATION_KEYS[i], cx)).ok();
            });
            let chosen = speech.device.as_deref().unwrap_or(dictation::DEFAULT_ID);
            let at = self.mics.iter().position(|m| m.id.as_ref() == chosen).unwrap_or(0);
            let ids: Vec<SharedString> = self.mics.iter().map(|m| m.id.clone()).collect();
            let mics = atelier_ui::Select::new("dictation-mic", self.mics.iter().map(|m| m.label.clone())).selected(Some(at)).on_change(move |i, _, cx| {
                let id = ids.get(i).cloned();
                mic_pane
                    .update(cx, |p, cx| p.choose_dictation(|d| d.device = id.filter(|id| id != dictation::DEFAULT_ID).map(|id| id.to_string()), cx))
                    .ok();
            });
            let key_gist = match speech.key {
                Some(atelier_voice::hotkey::Key::Fn) if cfg!(target_os = "macos") => {
                    "Hold it to talk; tap it to keep talking, and tap again to stop. In System Settings, Keyboard, set \"Press 🌐 key to\" to \"Do nothing\", or macOS takes the key for its own dictation."
                }
                Some(_) if cfg!(target_os = "macos") => "Hold it to talk; tap it to keep talking, and tap again to stop. A shortcut with the key still works.",
                Some(_) => "Hold it to talk; tap it to keep talking, and tap again to stop. The key works on macOS for now.",
                None => "The microphone button still dictates.",
            };
            div()
                .flex()
                .flex_col()
                .child(row("Dictation key", keys.into_any_element()))
                .child(div().debug_selector(|| "dictation-key-gist".into()).pb(px(8.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(key_gist))
                .child(row("Microphone", div().debug_selector(|| "dictation-mic".into()).w(px(260.)).child(mics).into_any_element()))
                .child(row(
                    "Hold the microphone button to record",
                    atelier_ui::Switch::new("dictation-hold", speech.hold)
                        .debug_name("dictation-hold")
                        .on_change(move |on, _, cx| {
                            hold_pane.update(cx, |p, cx| p.choose_dictation(|d| d.hold = on, cx)).ok();
                        })
                        .into_any_element(),
                ))
        };
        let keys_list = div().flex().flex_col().children(key_rows);
        let body = match self.section {
            Section::Appearance => appearance.into_any_element(),
            Section::Sidebar => sidebar.into_any_element(),
            Section::Agents => agents.into_any_element(),
            Section::Dictation => dictation_pane.into_any_element(),
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
                    .rounded(atelier_ui::theme::radius::md())
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
