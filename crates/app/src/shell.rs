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
    popover::{Hang, Popover},
    theme::{ActiveTheme, radius},
    typography::{FONT_FAMILY, TextSize},
};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, ParentElement,
    PathPromptOptions, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea, actions,
    base::{ResizableState, ResizeHandleRenderer, h_resizable, resizable_panel},
    div, prelude::FluentBuilder, px,
};
use lathe_project::LocalProject;
use lathe_settings::Location;

use std::collections::BTreeMap;

use beui::{
    agent_panels::AgentPanels,
    panel_types::{Layout, PanelsEvent, PanelsState},
    sidebar::{Sidebar, SidebarEvent},
};
use gpui_kit::AnyElement;

use crate::{
    agent_session::{self, AgentSession},
    agents_view,
    ssh_form::{Phase, SshForm, SshFormEvent},
    editor_pane::editor_pane,
    open_project::{Git, Listing, OpenProject, ProjectEvent},
    review_pane::Scope,
    tree_view::tree_view,
};

actions!(lathe, [OpenFolder, OpenRemote, NewSession, Save, CloseTab, ToggleSidebar, ToggleRight]);

/// The title bar's height, and the room the macOS window buttons take at its left.
pub const TITLE_BAR: f32 = 38.;
const TRAFFIC_LIGHTS: f32 = if cfg!(target_os = "macos") { 78. } else { 12. };

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-shift-o", OpenRemote, None),
        KeyBinding::new("secondary-shift-O", OpenRemote, None),
        KeyBinding::new("secondary-s", Save, None),
        KeyBinding::new("secondary-n", NewSession, None),
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
    /// "Open over SSH…", while it is open.
    ssh: Option<(Entity<SshForm>, Subscription)>,
    focus: FocusHandle,
    /// The projects and their sessions.
    agents_sidebar: Entity<Sidebar>,
    /// The open sessions' panels.
    panels: Entity<AgentPanels>,
    /// Names the reader gave sessions, by the agent's id.
    names: BTreeMap<String, String>,
    /// With `LATHE_FRAMES=1`, times every frame.
    meter: Option<Rc<std::cell::RefCell<crate::frame_meter::Meter>>>,
    /// The widths of the sidebar, the agent panel and the right pane.
    splits: Entity<ResizableState>,
    /// The right pane's width before a review widened it, to give back when the review closes.
    before_review: Option<Pixels>,
    _subscriptions: Vec<Subscription>,
}

/// The right pane's width a review opens at, room for its tree beside the file: less when the window
/// has not got it, since the agent panel keeps its least width.
const REVIEW_WIDTH: f32 = 860.;
/// The agent panel's least width, which a reader can drag it to.
const AGENT_LEAST: f32 = 320.;
/// What a review leaves the agent panel: a session panel at its default width, and its margins.
const AGENT_BESIDE_REVIEW: f32 = beui::panel_layout::DEFAULT_WIDTH + 2. * beui::panel_layout::GAP + 4.;

impl Shell {
    pub fn new(saved: &lathe_settings::Settings, cx: &mut Context<Self>) -> Self {
        let agents_sidebar = cx.new(Sidebar::new);
        let panels = cx.new(|cx| {
            let mut panels = AgentPanels::new(cx);
            let layout = if saved.panels.single { Layout::Single } else { Layout::SideBySide };
            let widths = saved.panels.widths.iter().map(|(id, w)| (SharedString::from(id.clone()), *w)).collect();
            panels.restore(PanelsState { layout, grouped: saved.panels.grouped, widths }, cx);
            panels
        });
        Self {
            projects: Vec::new(),
            active: 0,
            sidebar: true,
            right: true,
            recent: saved.recent.clone(),
            said: None,
            finder: None,
            ssh: None,
            focus: cx.focus_handle(),
            agents_sidebar,
            panels,
            names: saved.session_names.clone(),
            meter: crate::frame_meter::enabled().then(Default::default),
            splits: cx.new(|_| ResizableState::default()),
            before_review: None,
            _subscriptions: Vec::new(),
        }
    }

    /// Hears the sidebar and the panels. Called once the window exists.
    pub fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = self.agents_sidebar.clone();
        let panels = self.panels.clone();
        self._subscriptions.push(cx.subscribe_in(&sidebar, window, Self::sidebar_event));
        self._subscriptions.push(cx.subscribe_in(&panels, window, Self::panels_event));
    }

    /// The sidebar and the panels, drawn again from the projects as they are now.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let projects = agents_view::sidebar(&self.projects, &self.names, cx);
        let (panels, order) = agents_view::panels(&self.projects, cx);
        let now = agent_session::now();
        self.agents_sidebar.update(cx, |s, cx| s.set_projects(projects, now, cx));
        self.panels.update(cx, |p, cx| p.set_panels(panels, order, cx));
        cx.notify();
    }

    fn project_by_id(&self, id: &str, cx: &App) -> Option<usize> {
        self.projects.iter().position(|p| agents_view::project_id(p.read(cx)).as_ref() == id)
    }

    /// The open session keyed `key`, and the index of its project.
    fn session_by_key(&self, key: &str, cx: &App) -> Option<(usize, Entity<AgentSession>)> {
        self.projects.iter().enumerate().find_map(|(i, p)| {
            p.read(cx).sessions.iter().find(|s| s.read(cx).key.as_ref() == key).map(|s| (i, s.clone()))
        })
    }

    fn new_session(&mut self, project: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.projects.get(project).cloned() else { return };
        let session = p.update(cx, |p, cx| p.open_session(None, window, cx));
        self.show_session(project, &session, window, cx);
    }

    /// Makes `session` the panel in front, and its project the one the tree and the editor show.
    fn show_session(&mut self, project: usize, session: &Entity<AgentSession>, window: &mut Window, cx: &mut Context<Self>) {
        self.active = project;
        self.sync(cx);
        let key = session.read(cx).key.clone();
        self.panels.update(cx, |p, cx| p.activate(&key, cx));
        self.seen(&key, cx);
        let composer = session.read(cx).composer.clone();
        composer.read(cx).focus_handle(cx).focus(window, cx);
    }

    /// The session keyed `key` is the one the reader looks at; no other is.
    fn seen(&mut self, key: &str, cx: &mut Context<Self>) {
        for p in &self.projects {
            for s in p.read(cx).sessions.clone() {
                s.update(cx, |s, cx| s.set_seen(s.key.as_ref() == key, cx));
            }
        }
        self.sync(cx);
    }

    fn sidebar_event(&mut self, _: &Entity<Sidebar>, event: &SidebarEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            SidebarEvent::Open { project, session } => {
                let Some(at) = self.project_by_id(project, cx) else { return };
                match agents_view::pick(session) {
                    agents_view::Pick::Open(key) => {
                        if let Some((at, session)) = self.session_by_key(&key, cx) {
                            self.show_session(at, &session, window, cx);
                        }
                    }
                    agents_view::Pick::Past(id) => {
                        let p = self.projects[at].clone();
                        let title = p.read(cx).past.iter().find(|s| s.id == id).map(|s| s.title.clone()).unwrap_or_default();
                        let title = self.names.get(&id.0).cloned().unwrap_or(title);
                        let session = p.update(cx, |p, cx| p.open_session(Some((id.clone(), title.into())), window, cx));
                        if let Some(name) = self.names.get(&id.0) {
                            session.update(cx, |s, _| s.name = Some(name.clone().into()));
                        }
                        self.show_session(at, &session, window, cx);
                    }
                }
            }
            SidebarEvent::NewSession { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.new_session(at, window, cx);
                }
            }
            SidebarEvent::RevealProject { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.active = at;
                    self.sidebar = true;
                    cx.notify();
                }
            }
            SidebarEvent::CopyPath { project } => cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(project.to_string())),
            SidebarEvent::CloseProject { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.projects.remove(at);
                    self.active = self.active.min(self.projects.len().saturating_sub(1));
                    self.sync(cx);
                }
            }
            // A remote project reconnects by itself; Retry says so.
            SidebarEvent::Retry { .. } => self.say("Reconnecting on its own; it retries every few seconds.".into(), cx),
        }
    }

    fn panels_event(&mut self, _: &Entity<AgentPanels>, event: &PanelsEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event {
            PanelsEvent::Activated(key) => {
                if let Some((at, _)) = self.session_by_key(key, cx) {
                    self.active = at;
                }
                self.seen(key, cx);
            }
            PanelsEvent::Closed(key) => {
                if let Some((at, _)) = self.session_by_key(key, cx) {
                    let key = key.to_string();
                    self.projects[at].update(cx, |p, cx| p.close_session(&key, cx));
                    self.sync(cx);
                }
            }
            PanelsEvent::StateChanged => {
                let state = self.panels.read(cx).state();
                let panels = lathe_settings::Panels {
                    single: state.layout == Layout::Single,
                    grouped: state.grouped,
                    widths: state.widths.iter().map(|(id, w)| (id.to_string(), *w)).collect(),
                };
                if let Some(path) = lathe_settings::path() {
                    cx.background_spawn(async move {
                        if let Err(error) = lathe_settings::update(&path, |s| s.panels = panels) {
                            eprintln!("could not save the panels: {error}");
                        }
                    })
                    .detach();
                }
            }
        }
    }

    fn new_session_key(&mut self, _: &NewSession, window: &mut Window, cx: &mut Context<Self>) {
        if !self.projects.is_empty() {
            self.new_session(self.active, window, cx);
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

    fn open_ssh_form(&mut self, _: &OpenRemote, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.new(|cx| SshForm::new(Vec::new(), window, cx));
        // ~/.ssh/config is read off the UI thread; the form fills its hosts in when it has them.
        let reading = cx.background_spawn(async { lathe_remote::ssh::known_hosts() });
        let filling = form.downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let hosts = reading.await;
            _ = filling.update_in(cx, |f, window, cx| f.set_hosts(hosts, window, cx));
        })
        .detach();
        let events = cx.subscribe_in(&form, window, |this, _, event: &SshFormEvent, window, cx| match event {
            SshFormEvent::Connect { host, path } => this.open_remote(host.clone(), path.clone(), window, cx),
            SshFormEvent::Cancel => this.close_ssh(window, cx),
        });
        form.read(cx).focus_handle(cx).focus(window, cx);
        self.ssh = Some((form, events));
        cx.notify();
    }

    /// Opens `path` on `host` over ssh: the host is probed and given lathe-remote if need be, all on
    /// a background thread, and the form, when open, shows each step and any failure.
    pub fn open_remote(&mut self, host: String, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let (tx, mut steps) = futures_channel::mpsc::unbounded::<String>();
        let connecting = {
            let (host, path) = (host.clone(), path.clone());
            cx.background_spawn(async move { lathe_remote::ssh::connect(&host, &path, &|line| drop(tx.unbounded_send(line))) })
        };
        cx.spawn_in(window, async move |this, cx| {
            use futures_util::StreamExt;
            let mut connecting = std::pin::pin!(connecting);
            let connected = loop {
                let step = std::pin::pin!(steps.next());
                match futures_util::future::select(step, connecting.as_mut()).await {
                    futures_util::future::Either::Left((Some(line), _)) => {
                        _ = this.update(cx, |this, cx| this.form_phase(Phase::Connecting(line.into()), cx));
                    }
                    futures_util::future::Either::Left((None, _)) => break connecting.await,
                    futures_util::future::Either::Right((connected, _)) => break connected,
                }
            };
            _ = this.update_in(cx, |this, window, cx| match connected {
                Ok(project) => {
                    this.ssh = None;
                    let location = Location::Ssh { host, path: PathBuf::from(path) };
                    this.add(location, Arc::new(project), window, cx);
                }
                Err(error) => {
                    let why = error.to_string();
                    if this.ssh.is_some() {
                        this.form_phase(Phase::Failed(why.into()), cx);
                    } else {
                        this.say(format!("Could not open {path} on {host}: {why}"), cx);
                    }
                }
            });
        })
        .detach();
    }

    fn form_phase(&mut self, phase: Phase, cx: &mut Context<Self>) {
        if let Some((form, _)) = &self.ssh {
            form.update(cx, |f, cx| {
                f.phase = phase;
                cx.notify();
            });
        }
    }

    /// Gives the right pane the width a review wants, taken from the agent panel while a session panel
    /// still fits in it.
    fn widen_for_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sizes = self.splits.read(cx).sizes().clone();
        let (Some(&right), Some(last)) = (sizes.last(), sizes.len().checked_sub(1)) else { return };
        let others = sizes[..last].iter().fold(px(0.), |a, b| a + *b);
        let sidebar = if self.sidebar { sizes[0] } else { px(0.) };
        let room = right + others - sidebar - px(AGENT_BESIDE_REVIEW);
        let want = px(REVIEW_WIDTH).min(room);
        if right < want {
            self.before_review.get_or_insert(right);
            self.splits.update(cx, |s, cx| s.resize_panel(last, want, window, cx));
        }
    }

    /// Opens `path` of `project` in the editor, and makes that project the one shown.
    fn open_in(&mut self, project: &Entity<OpenProject>, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.projects.iter().position(|p| p == project) {
            self.active = i;
        }
        project.update(cx, |p, cx| p.open_file(path, window, cx));
        cx.notify();
    }

    fn add(&mut self, location: Location, project: Arc<dyn lathe_project::Project>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.projects.iter().position(|p| p.read(cx).location == location) {
            self.active = i;
            cx.notify();
            return;
        }
        let entity = cx.new(|cx| OpenProject::new(location.clone(), project, window, cx));
        self._subscriptions.push(cx.subscribe_in(&entity, window, |this, project, event: &ProjectEvent, window, cx| match event {
            ProjectEvent::Said(line) => this.say(line.to_string(), cx),
            ProjectEvent::Open(path) => this.open_in(project, path, window, cx),
            ProjectEvent::Review { session, turn, path } => {
                if let Some(i) = this.projects.iter().position(|p| p == project) {
                    this.active = i;
                }
                let scope = turn.map_or(Scope::Whole, Scope::Turn);
                project.update(cx, |p, cx| p.open_review(session.clone(), scope, path.as_deref(), window, cx));
                this.right = true;
                this.widen_for_review(window, cx);
                cx.notify();
            }
            ProjectEvent::ReviewClosed => {
                if let Some(width) = this.before_review.take() {
                    let last = this.splits.read(cx).sizes().len().saturating_sub(1);
                    this.splits.update(cx, |s, cx| s.resize_panel(last, width, window, cx));
                }
            }
            ProjectEvent::Sessions => this.sync(cx),
            ProjectEvent::Renamed { id, name } => {
                this.names.insert(id.0.clone(), name.to_string());
                let (id, name) = (id.0.clone(), name.to_string());
                if let Some(path) = lathe_settings::path() {
                    cx.background_spawn(async move {
                        if let Err(error) = lathe_settings::update(&path, |s| drop(s.session_names.insert(id, name))) {
                            eprintln!("could not save the name: {error}");
                        }
                    })
                    .detach();
                }
                this.sync(cx);
            }
        }));
        // A project's branch or link changing redraws its sidebar header.
        self._subscriptions.push(cx.observe(&entity, |this, _, cx| this.sync(cx)));
        self.projects.push(entity);
        self.active = self.projects.len() - 1;
        self.remember(location, cx);
        self.sync(cx);
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

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.active().cloned() {
            p.update(cx, |p, cx| p.save_asking(window, cx));
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

    /// Closes "Open over SSH…", and gives focus back here.
    fn close_ssh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ssh.take().is_some() {
            self.focus.focus(window, cx);
        }
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
                    Location::Ssh { host, path } => this.open_remote(host.clone(), path.display().to_string(), window, cx),
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
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                Button::new("open-folder")
                                    .label("Open Folder…")
                                    .size(ButtonSize::Md)
                                    .variant(ButtonVariant::Primary)
                                    .cap(keys::cap("⌘o"))
                                    .on_click(cx.listener(|this, _, window, cx| this.open_folder(&OpenFolder, window, cx))),
                            )
                            .child(
                                Button::new("open-remote")
                                    .label("Open over SSH…")
                                    .size(ButtonSize::Md)
                                    .variant(ButtonVariant::Secondary)
                                    .cap(keys::cap("⌘⇧o"))
                                    .on_click(cx.listener(|this, _, window, cx| this.open_ssh_form(&OpenRemote, window, cx))),
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
        let heading = |words: SharedString| div().px(px(14.)).pt(px(10.)).pb(px(4.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(words);
        let tree = self.active().map(|p| tree_view(p, cx));
        let tree_heading: SharedString = match self.active() {
            Some(p) => format!("Files in {}", p.read(cx).name()).into(),
            None => "Files".into(),
        };
        let current = theme.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            // The projects and their sessions, then the front project's files, each half the height.
            .child(div().flex_1().min_h_0().child(self.agents_sidebar.clone()))
            .child(heading(tree_heading))
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

    /// The open sessions' panels, or, with none open, a way to start one.
    fn agent_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.projects.iter().any(|p| !p.read(cx).sessions.is_empty());
        if open {
            // The strip scrolls its columns sideways; they must not draw under the sidebar.
            return div().size_full().overflow_hidden().child(self.panels.clone()).into_any_element();
        }
        let muted = cx.theme().muted_foreground;
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(div().text_size(TextSize::Sm.font_size()).child("No session open"))
            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("Start one, or open a past one from the sidebar."))
            .child(
                Button::new("new-session")
                    .label("New session")
                    .size(ButtonSize::Md)
                    .variant(ButtonVariant::Primary)
                    .cap(keys::cap("⌘n"))
                    .on_click(cx.listener(|this, _, window, cx| this.new_session_key(&NewSession, window, cx))),
            )
            .into_any_element()
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
                Git::Branch(b) => match p.dirty {
                    Some(0) => format!("{b}, clean").into(),
                    Some(1) => format!("{b}, 1 file changed").into(),
                    Some(n) => format!("{b}, {n} files changed").into(),
                    None => b.clone(),
                },
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self.root(window, cx);
        match self.meter.clone() {
            Some(meter) => crate::frame_meter::Timed { child: root, meter }.into_any_element(),
            None => root,
        }
    }
}

impl Shell {
    fn root(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // The side panes keep their width when the other hides; the agent panel takes what is left.
        let body = match self.active().cloned() {
            None => self.start_screen(cx).into_any_element(),
            Some(project) => h_resizable("shell-splits")
                .with_state(&self.splits)
                .with_handle_appearance(borderless_handle(&theme))
                .child(resizable_panel().visible(self.sidebar).size(px(260.)).size_range(px(180.)..px(480.)).flex_none().child(self.sidebar(cx)))
                .child(resizable_panel().size_range(px(AGENT_LEAST)..px(4000.)).child(self.agent_panel(cx)))
                .child(
                    resizable_panel()
                        .visible(self.right)
                        .size(px(560.))
                        .size_range(px(320.)..px(2400.))
                        .flex_none()
                        .child(div().size_full().pr(px(8.)).pb(px(2.)).child(match project.read(cx).review.as_ref() {
                            // The review draws its own cards on the page.
                            Some((pane, _)) => div().size_full().pt(px(6.)).child(pane.clone()),
                            None => div().size_full().pt(px(6.)).rounded(radius::LG).bg(theme.card).child(editor_pane(&project, cx)),
                        })),
                )
                .into_any_element(),
        };
        let link_down = self.active().and_then(|p| match &p.read(cx).link {
            lathe_project::Link::Down(why) => Some((p.read(cx).location.place(), why.clone())),
            lathe_project::Link::Up => None,
        });
        let banner = link_down.map(|(place, why)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .mx(px(8.))
                .mb(px(6.))
                .px(px(12.))
                .py(px(6.))
                .rounded(radius::LG)
                .bg(theme.card_strong)
                .text_size(TextSize::Xs.font_size())
                .child(beui::spinner::Spinner::new("reconnecting").size(px(12.)).color(theme.warning))
                .child(format!("Lost {place} ({why}). Reconnecting; your unsaved edits are kept here."))
        });
        div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::open_ssh_form))
            .on_action(cx.listener(Self::new_session_key))
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
            .children(banner)
            .child(div().flex().flex_1().min_h_0().child(body))
            .child(self.status_line(cx))
            // Both on the shared Popover: a press outside only closes, as do Escape and Tab.
            .children(self.ssh.as_ref().map(|(form, _)| {
                let this = cx.entity().downgrade();
                let focus = form.read(cx).focus_handle(cx);
                Popover::new("open-over-ssh")
                    .open(true)
                    .hang(Hang::Centre(TITLE_BAR + 60.))
                    .panel_focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_ssh(window, cx))))
                    .child(form.clone())
            }))
            .children(self.finder.as_ref().map(|(finder, _, _)| {
                let this = cx.entity().downgrade();
                let focus = gpui_kit::Focusable::focus_handle(finder, cx);
                Popover::new("go-to-file")
                    .open(true)
                    .hang(Hang::Centre(TITLE_BAR + 12.))
                    .height(360.)
                    .panel_focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_finder(true, window, cx))))
                    .child(finder.clone())
            }))
            .into_any_element()
    }
}

/// Borderless: a split's handle paints nothing at rest, and a wash while it is dragged.
fn borderless_handle(theme: &beui::Theme) -> ResizeHandleRenderer {
    let wash = theme.muted_hover();
    Rc::new(move |handle, _, _| Some(div().size_full().when(handle.is_active(), |d| d.bg(wash)).into_any_element()))
}
