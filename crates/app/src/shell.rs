//! The window: a title bar that is part of the page, the sidebar (the projects in this window, the
//! active project's sessions and its file tree), the agent panel in the middle, the editor on the
//! right, and the status line at the foot. With no project open, the start screen fills the window.
//!
//! Keys: ⌘O opens a folder, ⌘S saves, ⌘W closes the tab, and from GitQuiet's table, `t` goes to a
//! file (while nothing is being typed) and ⌘B and ⌘⇧B hide and show the left and the right pane.

use std::{path::PathBuf, rc::Rc, sync::Arc};

use beui::{
    button::{Button, ButtonSize, ButtonVariant},
    file_icon::FileIcon,
    finder::{Filter, Finder, FinderEvent, FinderItem},
    keys::{self, Command, Press},
    theme::{ActiveTheme, radius},
    typography::{FONT_FAMILY, TextSize},
};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, ParentElement,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea, actions,
    base::{ResizeHandleRenderer, h_resizable, resizable_panel},
    deferred, div, prelude::FluentBuilder, px,
};
use lathe_project::LocalProject;
use lathe_settings::Location;

use crate::{
    editor_pane::editor_pane,
    open_project::{Git, Listing, OpenProject, ProjectEvent},
    tree_view::tree_view,
};

actions!(lathe, [OpenFolder, Save, CloseTab, ToggleSidebar, ToggleRight]);

/// The title bar's height, and the room the macOS window buttons take at its left.
pub const TITLE_BAR: f32 = 38.;
const TRAFFIC_LIGHTS: f32 = if cfg!(target_os = "macos") { 78. } else { 12. };

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-s", Save, None),
        KeyBinding::new("secondary-w", CloseTab, None),
        KeyBinding::new("secondary-b", ToggleSidebar, None),
        // A shifted combo arrives with the letter either way, depending on the platform.
        KeyBinding::new("secondary-shift-b", ToggleRight, None),
        KeyBinding::new("secondary-shift-B", ToggleRight, None),
    ]);
}

pub struct Shell {
    projects: Vec<Entity<OpenProject>>,
    active: usize,
    sidebar: bool,
    right: bool,
    recent: Vec<Location>,
    /// The last thing a project or the shell said, for the status line.
    said: Option<SharedString>,
    /// Go to file, while it is open: the finder and the paths its rows stand for.
    finder: Option<(Entity<Finder>, Vec<String>, Subscription)>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(recent: Vec<Location>, cx: &mut Context<Self>) -> Self {
        Self {
            projects: Vec::new(),
            active: 0,
            sidebar: true,
            right: true,
            recent,
            said: None,
            finder: None,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn active(&self) -> Option<&Entity<OpenProject>> {
        self.projects.get(self.active)
    }

    /// Opens the folder at `path`, or shows it when this window has it open already.
    pub fn open_local(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let opening = cx.background_spawn(async move { LocalProject::open(path) });
        cx.spawn_in(window, async move |this, cx| {
            let opened = opening.await;
            _ = this.update_in(cx, |this, window, cx| match opened {
                Ok(project) => {
                    let location = Location::Local { path: lathe_project::Project::root(&project).to_path_buf() };
                    this.add(location, Arc::new(project), window, cx);
                }
                Err(error) => this.say(format!("Could not open the folder: {error}"), cx),
            });
        })
        .detach();
    }

    fn add(&mut self, location: Location, project: Arc<dyn lathe_project::Project>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.projects.iter().position(|p| p.read(cx).location == location) {
            self.active = i;
            cx.notify();
            return;
        }
        let entity = cx.new(|cx| OpenProject::new(location.clone(), project, window, cx));
        self._subscriptions.push(cx.subscribe(&entity, |this, _, event: &ProjectEvent, cx| match event {
            ProjectEvent::Said(line) => this.say(line.to_string(), cx),
        }));
        self.projects.push(entity);
        self.active = self.projects.len() - 1;
        self.remember(location, cx);
        cx.notify();
    }

    /// Puts `location` first in the recent list, here and in the settings file.
    fn remember(&mut self, location: Location, cx: &mut Context<Self>) {
        let mut settings = lathe_settings::Settings { recent: std::mem::take(&mut self.recent), ..Default::default() };
        settings.opened(location.clone());
        self.recent = settings.recent;
        if let Some(path) = lathe_settings::path() {
            cx.background_spawn(async move {
                if let Err(error) = lathe_settings::update(&path, |s| s.opened(location)) {
                    eprintln!("could not remember the project: {error}");
                }
            })
            .detach();
        }
    }

    fn say(&mut self, line: String, cx: &mut Context<Self>) {
        self.said = Some(line.into());
        cx.notify();
    }

    fn open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Open".into()) });
        cx.spawn_in(window, async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Ok(_)) => return,
                Ok(Err(error)) => {
                    _ = this.update(cx, |this, cx| this.say(format!("The folder picker did not open: {error}"), cx));
                    return;
                }
                Err(_) => return,
            };
            _ = this.update_in(cx, |this, window, cx| this.open_local(path, window, cx));
        })
        .detach();
    }

    fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.active().cloned() {
            p.update(cx, |p, cx| p.save(cx));
        }
    }

    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.active().cloned() {
            p.update(cx, |p, cx| {
                if let Some(path) = p.tabs.active().map(str::to_string) {
                    p.close_asking(&path, window, cx);
                }
            });
        }
    }

    /// Tabs with unsaved edits, across every project in the window.
    pub fn unsaved(&self, cx: &App) -> usize {
        self.projects.iter().map(|p| p.read(cx).unsaved()).sum()
    }

    /// A key from GitQuiet's table. ⌘B and ⌘⇧B arrive as actions instead, so they work from the
    /// editor too; here, Go to file, and only while nothing is being typed.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let press = Press::from_keystroke(&event.keystroke);
        if press.secondary || keys::typing(window) {
            return;
        }
        if keys::read_now(&press, cx) == Some(Command::GoToFile) {
            cx.stop_propagation();
            self.go_to_file(window, cx);
        }
    }

    fn go_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.active().cloned() else { return };
        let Listing::Ready(tree) = &project.read(cx).listing else { return };
        let paths = tree.file_paths();
        let items = paths.iter().map(|p| FinderItem::new(p.clone(), "").icon(p.clone())).collect();
        let finder = cx.new(|cx| {
            let mut finder = Finder::new("Go to file", "Part of a path", Filter::Here, window, cx).command(Command::GoToFile);
            finder.set_items(items, cx);
            finder
        });
        let events = cx.subscribe_in(&finder, window, move |this, _, event: &FinderEvent, window, cx| match event {
            FinderEvent::Query(_) => {}
            FinderEvent::Pick(i) => {
                if let Some(path) = this.finder.as_ref().and_then(|(_, paths, _)| paths.get(*i).cloned()) {
                    project.update(cx, |p, cx| p.open_file(&path, window, cx));
                }
                this.close_finder(false, window, cx);
            }
            FinderEvent::Dismiss => this.close_finder(true, window, cx),
        });
        finder.read(cx).focus_handle(cx).focus(window, cx);
        self.finder = Some((finder, paths, events));
        cx.notify();
    }

    /// Closes Go to file. A pick hands focus to the file's editor; a dismissal gives it back here.
    fn close_finder(&mut self, refocus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.finder.take().is_some() && refocus {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        cx.notify();
    }

    fn toggle_right(&mut self, _: &ToggleRight, _: &mut Window, cx: &mut Context<Self>) {
        self.right = !self.right;
        cx.notify();
    }

    fn title_bar(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let (name, branch) = match self.active() {
            Some(p) => {
                let p = p.read(cx);
                (Some(p.name()), match &p.git {
                    Git::Branch(b) => Some(b.to_string()),
                    _ => None,
                })
            }
            None => (None, None),
        };
        div()
            .id("title-bar")
            .window_control_area(WindowControlArea::Drag)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(TITLE_BAR))
            .pl(px(TRAFFIC_LIGHTS))
            .pr(px(12.))
            .text_size(TextSize::Sm.font_size())
            .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(name.unwrap_or_else(|| "lathe".into())))
            .children(branch.map(|b| div().text_color(theme.muted_foreground).child(b)))
    }

    fn start_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let recent = self.recent.iter().enumerate().map(|(i, location)| {
            let open = location.clone();
            div()
                .id(("recent", i))
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(40.))
                .px(px(10.))
                .rounded(radius::LG)
                .cursor_pointer()
                .hover(|s| s.bg(theme.muted_hover()))
                .on_click(cx.listener(move |this, _, window, cx| match &open {
                    Location::Local { path } => this.open_local(path.clone(), window, cx),
                    Location::Ssh { .. } => this.say("Remote projects open from “Open over SSH…”.".into(), cx),
                }))
                .child(FileIcon::folder(&location.name(), false).size(px(16.)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(div().text_size(TextSize::Sm.font_size()).truncate().child(location.name()))
                        .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).truncate().child(location.place())),
                )
        });
        let has_recent = !self.recent.is_empty();
        div()
            .flex()
            .flex_1()
            .justify_center()
            .pt(px(120.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(20.))
                    .w(px(420.))
                    .child(div().text_size(TextSize::Lg.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Open a project"))
                    .child(
                        div().flex().gap(px(8.)).child(
                            Button::new("open-folder")
                                .label("Open Folder…")
                                .size(ButtonSize::Md)
                                .variant(ButtonVariant::Primary)
                                .cap(keys::cap("⌘o"))
                                .on_click(cx.listener(|this, _, window, cx| this.open_folder(&OpenFolder, window, cx))),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).pb(px(4.)).child("Recent"))
                            .when(!has_recent, |d| {
                                d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("Folders you open show here."))
                            })
                            .children(recent),
                    ),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let projects = self.projects.iter().enumerate().map(|(i, p)| {
            let name = p.read(cx).name();
            let shown = i == self.active;
            div()
                .id(("project", i))
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .px(px(8.))
                .mx(px(6.))
                .rounded(radius::MD)
                .cursor_pointer()
                .text_size(TextSize::Sm.font_size())
                .when(shown, |d| d.bg(theme.muted_hover()).font_weight(gpui_kit::FontWeight::MEDIUM))
                .hover(|s| s.bg(theme.muted_hover()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active = i;
                    cx.notify();
                }))
                .child(FileIcon::folder(&name, shown).size(px(14.)))
                .child(div().min_w_0().truncate().child(name))
        });
        let heading = |words: &'static str| div().px(px(14.)).pt(px(10.)).pb(px(4.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(words);
        let tree = self.active().map(|p| tree_view(p, cx));
        let current = theme.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(heading("Projects"))
            .children(projects)
            .child(heading("Sessions"))
            .child(div().px(px(14.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("No sessions yet."))
            .child(heading("Files"))
            .child(div().flex_1().min_h_0().children(tree))
            .child(div().flex_none().p(px(8.)).child(beui::theme_picker::theme_picker("theme", &current, |picked, cx| {
                let name = picked.name.to_string();
                if let Some(path) = lathe_settings::path() {
                    cx.background_spawn(async move {
                        if let Err(error) = lathe_settings::update(&path, |s| s.theme = Some(name)) {
                            eprintln!("could not save the theme: {error}");
                        }
                    })
                    .detach();
                }
            })))
    }

    fn agent_panel(&self, cx: &App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(div().text_size(TextSize::Sm.font_size()).child("No session"))
            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("An agent session for this project shows here."))
    }

    fn status_line(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mut parts: Vec<SharedString> = Vec::new();
        if let Some(p) = self.active() {
            let p = p.read(cx);
            parts.push(p.location.place().into());
            parts.push(match &p.git {
                Git::Unknown => "…".into(),
                Git::None => "No git repository".into(),
                Git::Branch(b) => b.clone(),
            });
            if let (Listing::Ready(tree), Some(took)) = (&p.listing, p.listed_in) {
                let files = match tree.files() {
                    1 => "1 file".to_string(),
                    n => format!("{n} files"),
                };
                parts.push(format!("{files}, listed in {} ms", took.as_millis()).into());
            }
            if let Some((_, buffer)) = p.active_buffer() {
                parts.extend(buffer.session.read(cx).status());
            }
        }
        parts.extend(self.said.clone());
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(14.))
            .h(px(26.))
            .px(px(12.))
            .text_size(TextSize::Xs.font_size())
            .text_color(muted)
            .overflow_hidden()
            .children(parts.into_iter().map(|part| div().flex_none().whitespace_nowrap().child(part)))
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // The side panes keep their width when the other hides; the agent panel takes what is left.
        let body = match self.active().cloned() {
            None => self.start_screen(cx).into_any_element(),
            Some(project) => h_resizable("shell-splits")
                .with_handle_appearance(borderless_handle(&theme))
                .child(resizable_panel().visible(self.sidebar).size(px(260.)).size_range(px(180.)..px(480.)).flex_none().child(self.sidebar(cx)))
                .child(resizable_panel().size_range(px(320.)..px(4000.)).child(self.agent_panel(cx)))
                .child(
                    resizable_panel()
                        .visible(self.right)
                        .size(px(560.))
                        .size_range(px(320.)..px(2400.))
                        .flex_none()
                        .child(div().size_full().pr(px(8.)).pb(px(2.)).child(
                            div().size_full().pt(px(6.)).rounded(radius::LG).bg(theme.card).child(editor_pane(&project, cx)),
                        )),
                )
                .into_any_element(),
        };
        div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_right))
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(FONT_FAMILY)
            .relative()
            .child(self.title_bar(cx))
            .child(div().flex().flex_1().min_h_0().child(body))
            .child(self.status_line(cx))
            .children(self.finder.as_ref().map(|(finder, _, _)| {
                deferred(div().absolute().top(px(TITLE_BAR + 12.)).left_0().right_0().flex().justify_center().child(finder.clone()))
                    .with_priority(1)
            }))
    }
}

/// Borderless: a split's handle paints nothing at rest, and a wash while it is dragged.
fn borderless_handle(theme: &beui::Theme) -> ResizeHandleRenderer {
    let wash = theme.muted_hover();
    Rc::new(move |handle, _, _| Some(div().size_full().when(handle.is_active(), |d| d.bg(wash)).into_any_element()))
}
