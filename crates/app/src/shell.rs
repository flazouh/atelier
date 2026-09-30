//! The window: a title bar that is part of the page, the sidebar (the projects in this window, the
//! active project's sessions and its file tree), the agent panel in the middle, the editor on the
//! right, and the status line at the foot. With no project open, the start screen fills the window.
//!
//! Keys: ⌘O opens a folder, ⌘S saves, ⌘W closes the tab, and from GitQuiet's table, `t` goes to a
//! file (while nothing is being typed) and ⌘B and ⌘⇧B hide and show the left and the right pane.

use std::{path::PathBuf, rc::Rc, sync::Arc};

use beui::{
    PressStop,
    button::{Button, ButtonSize, ButtonVariant},
    file_icon::FileIcon,
    finder::{Filter, Finder, FinderEvent, FinderItem},
    segmented::{Segment, Segmented},
    keys::{self, Command, Press},
    modal::Modal,
    popover::{Hang, Popover},
    theme::{ActiveTheme, radius},
    typography::{FONT_FAMILY, TextSize},
};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, ParentElement,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea, actions,
    div, prelude::FluentBuilder, px,
};
use lathe_project::LocalProject;
use lathe_settings::Location;

use std::collections::BTreeMap;
mod fit;
mod restore;
use fit::{Fit, Pane};

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
    open_project::{Listing, OpenProject, ProjectEvent},
    review_pane::Scope,
    tree_view::tree_view,
};

actions!(lathe, [OpenFolder, OpenRemote, NewSession, Save, CloseTab, ToggleSidebar, ToggleRight, PullRequests, OpenSettings, Quit]);

/// The title bar's height, and the room the macOS window buttons take at its left.
pub const TITLE_BAR: f32 = 38.;
const TRAFFIC_LIGHTS: f32 = if cfg!(target_os = "macos") { 78. } else { 12. };

pub fn bind_keys(cx: &mut App) {
    crate::ship::strip::bind_keys(cx);
    crate::ship::pull_form::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-shift-o", OpenRemote, None),
        KeyBinding::new("secondary-shift-O", OpenRemote, None),
        KeyBinding::new("secondary-s", Save, None),
        KeyBinding::new("secondary-n", NewSession, None),
        KeyBinding::new("secondary-w", CloseTab, None),
        KeyBinding::new("secondary-b", ToggleSidebar, None),
        // A shifted combo arrives with the letter either way, depending on the platform.
        KeyBinding::new("secondary-shift-b", ToggleRight, None),
        KeyBinding::new("secondary-shift-B", ToggleRight, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        KeyBinding::new("secondary-shift-p", PullRequests, None),
        KeyBinding::new("secondary-shift-P", PullRequests, None),
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
    /// The Settings pane, while it is open.
    settings: Option<(Entity<crate::settings_pane::SettingsPane>, Subscription)>,
    ssh: Option<(Entity<SshForm>, Subscription)>,
    /// The in-app folder picker, when the system has none.
    folder: Option<(Entity<beui::FolderPicker>, Subscription)>,
    focus: FocusHandle,
    /// The projects and their sessions.
    agents_sidebar: Entity<Sidebar>,
    /// The open sessions' panels.
    panels: Entity<AgentPanels>,
    /// Names the reader gave sessions, by the agent's id.
    names: BTreeMap<String, String>,
    /// With `LATHE_FRAMES=1`, times every frame.
    meter: Option<Rc<std::cell::RefCell<crate::frame_meter::Meter>>>,
    /// The sidebar's and the right pane's widths as the reader dragged them; the window's width may
    /// show them narrower (`fit::widths`).
    sidebar_width: f32,
    right_width: f32,
    /// Whether the sidebar shows in a window too narrow for it by default, after ⌘B.
    sidebar_in_medium: bool,
    /// The pane a narrow window shows.
    narrow: Pane,
    /// The right pane's width before a review widened it, to give back when the review closes.
    before_review: Option<f32>,
    /// Sessions open at the last quit, waiting for their project to open.
    restoring: Vec<lathe_settings::OpenSession>,
    /// Projects being opened: until they all arrive, a saved session may still find its project.
    opening: usize,
    /// The session in front at the last quit, by the agent's id.
    front: Option<String>,
    /// The open sessions and the one in front as the settings file has them, to write only a change.
    saved_open: (Vec<lathe_settings::OpenSession>, Option<String>),
    _subscriptions: Vec<Subscription>,
}

/// The right pane's width a review opens at, room for its tree beside the file: less when the window
/// has not got it, since the agent panel keeps its least width.
const REVIEW_WIDTH: f32 = 860.;
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
            folder: None,
            settings: None,
            focus: cx.focus_handle(),
            agents_sidebar,
            panels,
            names: saved.session_names.clone(),
            meter: crate::frame_meter::enabled().then(Default::default),
            sidebar_width: fit::SIDEBAR_DEFAULT,
            right_width: fit::RIGHT_DEFAULT,
            sidebar_in_medium: false,
            narrow: Pane::Session,
            restoring: Vec::new(),
            opening: 0,
            front: None,
            saved_open: (saved.open.clone(), saved.front.clone()),
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
        self.mark_open_session(cx);
        self.save_open(cx);
        cx.notify();
    }

    /// Keeps the open sessions and the one in front in the settings, off the UI thread, for the next
    /// launch. Nothing is written while sessions from the last quit still wait to open.
    fn save_open(&mut self, cx: &mut Context<Self>) {
        if !self.restoring.is_empty() {
            return;
        }
        let mut open = Vec::new();
        for project in &self.projects {
            let p = project.read(cx);
            for session in &p.sessions {
                let s = session.read(cx);
                if let Some(id) = &s.id {
                    open.push(lathe_settings::OpenSession { location: p.location.clone(), id: id.as_str().to_string(), title: s.shown_title().to_string() });
                }
            }
        }
        let front = self.panels.read(cx).active().and_then(|key| self.session_by_key(key, cx)).and_then(|(_, s)| s.read(cx).id.clone()).map(|id| id.as_str().to_string());
        let now = (open, front);
        if now == self.saved_open {
            return;
        }
        self.saved_open = now.clone();
        if let Some(path) = lathe_settings::path() {
            cx.background_spawn(async move {
                if let Err(error) = lathe_settings::update(&path, |s| (s.open, s.front) = now) {
                    eprintln!("could not keep the open sessions: {error}");
                }
            })
            .detach();
        }
    }

    /// The sessions open at the last quit: each opens again when its project opens. With `open_projects`
    /// (no folder named at launch), their projects open too; else only the named ones do.
    pub fn restore(&mut self, open: Vec<lathe_settings::OpenSession>, front: Option<String>, open_projects: bool, window: &mut Window, cx: &mut Context<Self>) {
        let places = restore::locations(&open);
        (self.restoring, self.front) = (open, front);
        if !open_projects {
            return;
        }
        for place in places {
            match place {
                Location::Local { path } => self.open_local(path, window, cx),
                Location::Ssh { host, path } => self.open_remote(host, path.display().to_string(), window, cx),
            }
        }
    }

    /// The sessions of project `at` that were open at the last quit, opened again; the one that was in
    /// front shows.
    fn reopen(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.projects.get(at).cloned() else { return };
        let location = project.read(cx).location.clone();
        let mine: Vec<lathe_settings::OpenSession> = restore::of(&self.restoring, &location).into_iter().cloned().collect();
        if mine.is_empty() {
            return;
        }
        self.restoring.retain(|s| s.location != location);
        let mut shown = None;
        for saved in mine {
            let id = lathe_agents::session::SessionId::new(saved.id.clone());
            let title = self.names.get(&saved.id).cloned().unwrap_or(saved.title.clone());
            let session = project.update(cx, |p, cx| p.open_session(Some((id, title.into())), window, cx));
            if let Some(name) = self.names.get(&saved.id) {
                session.update(cx, |s, _| s.name = Some(name.clone().into()));
            }
            if shown.is_none() || self.front.as_deref() == Some(saved.id.as_str()) {
                shown = Some(session);
            }
        }
        if let Some(session) = shown {
            self.show_session(at, &session, window, cx);
        }
    }

    /// Marks the row of the session in the active panel, in the sidebar.
    fn mark_open_session(&mut self, cx: &mut Context<Self>) {
        let active = self.panels.read(cx).active().cloned();
        self.agents_sidebar.update(cx, |s, cx| s.set_open(active, cx));
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
        self.narrow = Pane::Session;
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
            SidebarEvent::PullRequests { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.active = at;
                    self.show_pulls(window, cx);
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
                self.mark_open_session(cx);
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
        self.opening += 1;
        let opening = cx.background_spawn(async move { LocalProject::open(path) });
        cx.spawn_in(window, async move |this, cx| {
            let opened = opening.await;
            _ = this.update_in(cx, |this, window, cx| {
                match opened {
                    Ok(project) => {
                        let location = Location::Local { path: lathe_project::Project::root(&project).to_path_buf() };
                        this.add(location, Arc::new(project), window, cx);
                    }
                    Err(error) => this.say(format!("Could not open the folder: {error}"), cx),
                }
                this.opened_one(cx);
            });
        })
        .detach();
    }

    /// One project opening ended, opened or not. Once none is left opening, the sessions from the last
    /// quit that found no project stop waiting, and the open ones are kept from now on.
    fn opened_one(&mut self, cx: &mut Context<Self>) {
        self.opening = self.opening.saturating_sub(1);
        if self.opening == 0 && !self.restoring.is_empty() {
            self.restoring.clear();
            self.save_open(cx);
        }
    }

    /// ⌘,: the Settings pane over the window, or back to the window when it is open.
    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        let saved = lathe_settings::path().map(|p| lathe_settings::load(&p)).unwrap_or_default();
        let agents = lathe_agents::registry::agents()
            .into_iter()
            .map(|agent| crate::settings_pane::AgentRow {
                name: agent.name.into(),
                models: agent.backend.capabilities().models.into_iter().map(|m| SharedString::from(m.label)).collect(),
            })
            .collect();
        let pane = cx.new(|cx| crate::settings_pane::SettingsPane::new(&saved, agents, cx));
        let events = cx.subscribe_in(&pane, window, |this, _, event: &crate::settings_pane::SettingsEvent, window, cx| match event {
            crate::settings_pane::SettingsEvent::Close => {
                this.settings = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        });
        pane.read(cx).focus_handle(cx).focus(window, cx);
        self.settings = Some((pane, events));
        cx.notify();
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
            SshFormEvent::Connect { host } => this.browse_remote(host.clone(), window, cx),
            SshFormEvent::Cancel => this.close_ssh(window, cx),
        });
        form.read(cx).focus_handle(cx).focus(window, cx);
        self.ssh = Some((form, events));
        cx.notify();
    }

    /// Opens `path` on `host` over ssh: the host is probed and given lathe-remote if need be, all on
    /// a background thread, and the form, when open, shows each step and any failure.
    pub fn open_remote(&mut self, host: String, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.opening += 1;
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
            _ = this.update_in(cx, |this, window, cx| {
                match connected {
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
                }
                this.opened_one(cx);
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

    fn pull_requests_key(&mut self, _: &PullRequests, window: &mut Window, cx: &mut Context<Self>) {
        self.show_pulls(window, cx);
    }

    /// Shows or hides the active project's pull requests in the right pane, as wide as a review.
    fn show_pulls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.active().cloned() else { return };
        project.update(cx, |p, cx| p.toggle_pulls(window, cx));
        self.right = true;
        self.widen_for_review(window, cx);
        cx.notify();
    }

    /// Gives the right pane the width a review wants, taken from the agent panel while a session panel
    /// still fits in it.
    fn widen_for_review(&mut self, window: &mut Window, _: &mut Context<Self>) {
        let total = f32::from(window.viewport_size().width);
        let fit = Fit::of(total);
        if fit == Fit::Narrow {
            self.narrow = Pane::Right;
            return;
        }
        let sidebar = if self.sidebar_shown(fit) { self.sidebar_width } else { 0. };
        let want = REVIEW_WIDTH.min(total - sidebar - AGENT_BESIDE_REVIEW);
        if self.right_width < want {
            self.before_review.get_or_insert(self.right_width);
            self.right_width = want;
        }
    }

    /// Whether the sidebar shows at `fit`: by the reader's choice in a wide window, after ⌘B in a
    /// medium one, and as its own tab in a narrow one.
    fn sidebar_shown(&self, fit: Fit) -> bool {
        match fit {
            Fit::Wide => self.sidebar,
            Fit::Medium => self.sidebar_in_medium,
            Fit::Narrow => false,
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
            ProjectEvent::PullsShown => {
                this.right = true;
                this.widen_for_review(window, cx);
                cx.notify();
            }
            ProjectEvent::ReviewClosed => {
                if let Some(width) = this.before_review.take() {
                    this.right_width = width;
                }
                if this.narrow == Pane::Right {
                    this.narrow = Pane::Session;
                }
                cx.notify();
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
        self.reopen(self.projects.len() - 1, window, cx);
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
                Ok(Err(_)) => {
                    // No system picker (no desktop portal): the app's own, which browses the same folders.
                    _ = this.update_in(cx, |this, window, cx| this.open_folder_picker(window, cx));
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
    /// ⌘Q (Control-Q elsewhere): the app ends, after asking when a tab holds unsaved edits.
    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        let unsaved = self.unsaved(cx);
        if unsaved == 0 {
            crate::exit_log::quit();
            return cx.quit();
        }
        let tabs = if unsaved == 1 { "1 tab has".to_string() } else { format!("{unsaved} tabs have") };
        let answer = window.prompt(
            gpui_kit::PromptLevel::Warning,
            &format!("{tabs} unsaved changes."),
            Some("They are lost if you quit."),
            &["Quit Anyway", "Cancel"],
            cx,
        );
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                cx.update(|cx| {
                    crate::exit_log::quit();
                    cx.quit();
                });
            }
        })
        .detach();
    }

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

    /// The app's own folder picker, over this machine's folders. It starts in the home folder.
    fn open_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder_picker_over(FolderSource::Local, window, cx);
    }

    /// The folder picker over `source`'s folders: this machine's, or a host's through its project.
    fn open_folder_picker_over(&mut self, source: FolderSource, window: &mut Window, cx: &mut Context<Self>) {
        let recent: Vec<SharedString> = match &source {
            FolderSource::Local => Vec::new(),
            FolderSource::Remote { host, .. } => self
                .recent
                .iter()
                .filter_map(|l| match l {
                    Location::Ssh { host: h, path } if h == host => Some(SharedString::from(path.display().to_string())),
                    _ => None,
                })
                .take(5)
                .collect(),
        };
        let picker = cx.new(|cx| beui::FolderPicker::new("~/", window, cx).with_recent(recent));
        let events = cx.subscribe_in(&picker, window, move |this, picker, event: &beui::FolderPickerEvent, window, cx| match event {
            beui::FolderPickerEvent::Want(dir) => {
                let dir = dir.to_string();
                let listing = cx.background_spawn({
                    let (dir, source) = (dir.clone(), source.clone());
                    async move {
                        match source {
                            FolderSource::Local => lathe_project::read_local_dir(&dir),
                            FolderSource::Remote { project, .. } => project.read_dir(&dir),
                        }
                    }
                });
                let picker = picker.downgrade();
                cx.spawn_in(window, async move |_, cx| {
                    let answer = listing.await.map(|all| all.into_iter().map(|e| (SharedString::from(e.name), e.dir)).collect()).map_err(|e| folder_error(&e));
                    _ = picker.update_in(cx, |p, window, cx| p.show(&dir, answer, window, cx));
                })
                .detach();
            }
            beui::FolderPickerEvent::Choose(path) => {
                // The picker stays until the folder opens: a folder that will not open is said in the picker, with
                // the path as the reader typed it.
                match &source {
                    FolderSource::Local => match lathe_project::expand_home(path) {
                        Some(target) => {
                            let opening = cx.background_spawn(async move { LocalProject::open(target) });
                            let picker = picker.downgrade();
                            cx.spawn_in(window, async move |this, cx| {
                                let opened = opening.await;
                                _ = this.update_in(cx, |this, window, cx| match opened {
                                    Ok(project) => {
                                        this.close_folder_picker(window, cx);
                                        let location = Location::Local { path: lathe_project::Project::root(&project).to_path_buf() };
                                        this.add(location, Arc::new(project), window, cx);
                                    }
                                    Err(error) => {
                                        picker.update(cx, |p, cx| p.refuse(folder_error(&error), cx)).ok();
                                    }
                                });
                            })
                            .detach();
                        }
                        None => {
                            picker.update(cx, |p, cx| p.refuse(beui::FolderError::Missing, cx));
                        }
                    },
                    FolderSource::Remote { host, .. } => {
                        let (host, path) = (host.clone(), path.to_string());
                        picker.update(cx, |p, cx| p.working(Some(format!("Opening {path} on {host}…").into()), cx));
                        let connecting = {
                            let (host, path) = (host.clone(), path.clone());
                            cx.background_spawn(async move { lathe_remote::ssh::connect(&host, &path, &|_| {}) })
                        };
                        let picker = picker.downgrade();
                        cx.spawn_in(window, async move |this, cx| {
                            let connected = connecting.await;
                            _ = this.update_in(cx, |this, window, cx| match connected {
                                Ok(project) => {
                                    this.close_folder_picker(window, cx);
                                    this.add(Location::Ssh { host, path: PathBuf::from(path) }, Arc::new(project), window, cx);
                                }
                                Err(error) => {
                                    picker.update(cx, |p, cx| p.refuse(folder_error(&error), cx)).ok();
                                }
                            });
                        })
                        .detach();
                    }
                }
            }
            beui::FolderPickerEvent::Cancel => this.close_folder_picker(window, cx),
        });
        picker.update(cx, |p, cx| p.ask(cx));
        picker.read(cx).focus_handle(cx).focus(window, cx);
        self.folder = Some((picker, events));
        cx.notify();
    }

    /// Connects to `host` at its home folder, and offers its folders to choose from.
    fn browse_remote(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        let (tx, mut steps) = futures_channel::mpsc::unbounded::<String>();
        let connecting = {
            let host = host.clone();
            cx.background_spawn(async move { lathe_remote::ssh::connect_at_home(&host, &|line| drop(tx.unbounded_send(line))) })
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
                Ok((project, _home)) => {
                    this.ssh = None;
                    let project: Arc<dyn lathe_project::Project> = Arc::new(project);
                    this.open_folder_picker_over(FolderSource::Remote { host, project }, window, cx);
                }
                Err(error) => this.form_phase(Phase::Failed(error.to_string().into()), cx),
            });
        })
        .detach();
    }

    fn close_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder.take().is_some() {
            self.focus.focus(window, cx);
        }
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

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, window: &mut Window, cx: &mut Context<Self>) {
        match Fit::of(f32::from(window.viewport_size().width)) {
            Fit::Wide => self.sidebar = !self.sidebar,
            Fit::Medium => self.sidebar_in_medium = !self.sidebar_in_medium,
            Fit::Narrow => self.narrow = if self.narrow == Pane::Projects { Pane::Session } else { Pane::Projects },
        }
        cx.notify();
    }
    fn toggle_right(&mut self, _: &ToggleRight, window: &mut Window, cx: &mut Context<Self>) {
        match Fit::of(f32::from(window.viewport_size().width)) {
            Fit::Narrow => self.narrow = if self.narrow == Pane::Right { Pane::Session } else { Pane::Right },
            Fit::Medium | Fit::Wide => self.right = !self.right,
        }
        cx.notify();
    }

    /// Opens the session that needs the reader most: one waiting for a yes or no, then a question, then one finished and unseen.
    fn open_most_urgent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = self.agents_sidebar.clone();
        let Some((project, session)) = beui::most_urgent(sidebar.read(cx).projects()) else { return };
        self.sidebar_event(&sidebar, &SidebarEvent::Open { project, session }, window, cx);
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let counts = beui::counts_of(self.agents_sidebar.read(cx).projects());
        let this = cx.entity().downgrade();
        let theme = cx.theme().clone();
        let (name, branch) = match self.active() {
            Some(p) => {
                let p = p.read(cx);
                (Some(p.name()), p.git.branch().map(ToString::to_string))
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
            .child(div().flex_1().flex().justify_center().child(beui::SessionsIsland::new("sessions-island", counts).on_press(
                move |window, cx| drop(this.update(cx, |shell, cx| shell.open_most_urgent(window, cx))),
            )))
    }

    fn start_screen(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .press_stop(gpui_kit::ElementId::Name(format!("recent-focus-{i}").into()), radius::LG, window, cx)
                .on_click(cx.listener(move |this, _, window, cx| match &open {
                    Location::Local { path } => this.open_local(path.clone(), window, cx),
                    Location::Ssh { host, path } => this.open_remote(host.clone(), path.display().to_string(), window, cx),
                }))
                .child(match location {
                    Location::Ssh { .. } => beui::Icon::new(beui::IconName::Dns).size(px(16.)).color(muted).into_any_element(),
                    Location::Local { .. } => FileIcon::folder(&location.name(), false).size(px(16.)).into_any_element(),
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(div().text_size(TextSize::Sm.font_size()).truncate().child(location.name()))
                        .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).truncate().child(location.place())),
                )
        }).collect::<Vec<_>>();
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
                    .child(div().flex().child(beui::LatheMark::new(40.)))
                    .child(div().text_size(TextSize::Lg.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Open a project"))
                    .child(div().debug_selector(|| "first-launch-line".into()).mt(px(-12.)).text_size(TextSize::Sm.font_size()).text_color(muted).child(WHAT_LATHE_IS))
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
                    .children(self.said.clone().map(|words| {
                        // A long error wraps here, above Recent, and does not run off the status line.
                        div().debug_selector(|| "start-error".into()).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(words)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().debug_selector(|| "recent-heading".into()).text_size(TextSize::Xs.font_size()).text_color(muted).pb(px(4.)).child("Recent"))
                            .when(!has_recent, |d| {
                                d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("Folders you open show here."))
                            })
                            .children(recent),
                    ),
            )
    }

    /// The foot of the sidebar: the Settings entry, which is the one home of the theme.
    fn sidebar_foot(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        div().flex_none().p(px(8.)).child(
            Button::new("settings-entry")
                .debug_name("settings-entry")
                .icon(beui::IconName::Settings)
                .label("Settings")
                .variant(ButtonVariant::Ghost)
                .cap(keys::cap("⌘,"))
                .on_click(cx.listener(|this, _, window, cx| this.open_settings(&OpenSettings, window, cx))),
        )
        .into_any_element()
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
        div()
            .flex()
            .flex_col()
            .size_full()
            // The projects and their sessions, then the front project's files, each half the height.
            .child(div().flex_1().min_h_0().child(self.agents_sidebar.clone()))
            .child(heading(tree_heading))
            .child(div().flex_1().min_h_0().children(tree))
            .child(self.sidebar_foot(cx))
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

    /// The panes for the window's width: the three side by side, the two without the sidebar, or one at
    /// a time with tabs (docs/app.md, "Window widths").
    fn panes(&mut self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let total = f32::from(window.viewport_size().width);
        let fit = Fit::of(total);
        if fit == Fit::Narrow {
            return self.narrow_panes(project, window, cx);
        }
        let wants = fit::Wants {
            sidebar: self.sidebar_shown(fit).then_some(self.sidebar_width),
            right: self.right.then_some(self.right_width),
        };
        let widths = fit::widths(total, wants);
        // The strip lays its columns out from this width in this frame; the strip keeps 8 px each side.
        self.panels.update(cx, |p, cx| p.fit_to(widths.agent - 16., cx));
        let wash = cx.theme().muted_hover();
        let handle = move |edge: Edge| {
            let d = div().id(match edge {
                Edge::Sidebar => "edge-sidebar",
                Edge::Right => "edge-right",
            });
            let d = match edge {
                Edge::Sidebar => d.right(px(-4.)),
                Edge::Right => d.left(px(-4.)),
            };
            d.absolute()
                .top_0()
                .bottom_0()
                .w(px(8.))
                .cursor(gpui_kit::CursorStyle::ResizeLeftRight)
                .hover(|d| d.bg(wash))
                .on_drag(edge, |_, _, _, cx| cx.new(|_| gpui_kit::Empty))
        };
        div()
            .id("shell-panes")
            .flex()
            .size_full()
            .min_h_0()
            .on_drag_move::<Edge>(cx.listener(|this, event: &gpui_kit::DragMoveEvent<Edge>, _, cx| {
                let x = f32::from(event.event.position.x - event.bounds.origin.x);
                match event.drag(cx) {
                    Edge::Sidebar => this.sidebar_width = x.clamp(fit::SIDEBAR_LEAST, fit::SIDEBAR_MOST),
                    Edge::Right => this.right_width = (f32::from(event.bounds.size.width) - x).clamp(fit::RIGHT_LEAST, fit::RIGHT_MOST),
                }
                cx.notify();
            }))
            .children(widths.sidebar.map(|w| div().relative().flex_none().w(px(w)).h_full().child(self.sidebar(cx)).child(handle(Edge::Sidebar))))
            .child(div().flex_1().min_w_0().h_full().child(self.agent_panel(cx)))
            .children(widths.right.map(|w| div().relative().flex_none().w(px(w)).h_full().child(self.right_pane(project, cx)).child(handle(Edge::Right))))
            .into_any_element()
    }

    /// One pane at a time, with a tab for each: the sidebar, the sessions, and the editor or what
    /// stands in its place.
    fn narrow_panes(&mut self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let right = {
            let p = project.read(cx);
            if p.review.is_some() {
                "Review"
            } else if p.pulls.as_ref().is_some_and(|pulls| pulls.shown) {
                "Pull requests"
            } else {
                "Editor"
            }
        };
        let panes = [Pane::Projects, Pane::Session, Pane::Right];
        let segment = |label: &'static str, cap: Option<&'static str>| match cap {
            Some(cap) => Segment::new(label).cap(keys::cap(cap)),
            None => Segment::new(label),
        };
        let this = cx.entity();
        let tabs = div().flex().flex_none().items_center().px(px(8.)).h(px(44.)).child(
            Segmented::new(
                "narrow-panes",
                [segment("Projects", Some("⌘b")), segment("Session", None), segment(right, Some("⌘⇧b"))],
                panes.iter().position(|p| *p == self.narrow).unwrap_or(0),
            )
            .on_change(move |i, _, cx| {
                this.update(cx, |this, cx| {
                    this.narrow = panes[i];
                    cx.notify();
                })
            }),
        );
        let body = match self.narrow {
            Pane::Projects => self.sidebar(cx).into_any_element(),
            Pane::Session => {
                let total = f32::from(window.viewport_size().width);
                self.panels.update(cx, |p, cx| p.fit_to(total - 16., cx));
                self.agent_panel(cx)
            }
            Pane::Right => self.right_pane(project, cx),
        };
        div().flex().flex_col().size_full().min_h_0().child(tabs).child(div().flex_1().min_h_0().child(body)).into_any_element()
    }

    /// The right pane: the review, the pull requests, or the editor.
    fn right_pane(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let p = project.read(cx);
        let pulls = p.pulls.as_ref().filter(|pulls| pulls.shown).map(|pulls| pulls.hub.clone());
        let inner = match (p.review.as_ref(), pulls) {
            // The review and the pull requests draw their own cards on the page.
            (Some((pane, _)), _) => div().size_full().pt(px(6.)).child(pane.clone()),
            (None, Some(hub)) => div().size_full().pt(px(6.)).child(hub),
            (None, None) => div().size_full().pt(px(6.)).rounded(radius::LG).bg(theme.card).child(editor_pane(project, cx)),
        };
        div().size_full().pr(px(8.)).pb(px(2.)).child(inner).into_any_element()
    }

    fn status_line(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mut parts: Vec<SharedString> = Vec::new();
        if let Some(p) = self.active() {
            let p = p.read(cx);
            parts.push(p.location.place().into());
            parts.push(p.git.words(p.dirty));
            if let (Listing::Ready(tree), Some(took)) = (&p.listing, p.listed_in) {
                let files = match tree.files() {
                    1 => "1 file".to_string(),
                    n => format!("{n} files"),
                };
                parts.push(format!("{files}, listed in {} ms", took.as_millis()).into());
            }
            // The review's server while it shows, else the open tab's.
            match (&p.review, p.active_buffer()) {
                (Some((pane, _)), _) => parts.extend(pane.read(cx).status(cx)),
                (None, Some((_, buffer))) => parts.extend(buffer.session.read(cx).status()),
                (None, None) => {}
            }
        }
        // With no project open the start screen shows the line itself.
        if self.active().is_some() {
            parts.extend(self.said.clone());
        }
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
            // The place and the branch keep their width; the rest gives way with an ellipsis, never a clip.
            .children(parts.into_iter().enumerate().map(|(at, part)| {
                let d = div().whitespace_nowrap().child(part);
                if at < 2 { d.flex_none() } else { d.min_w_0().truncate() }
            }))
    }
}

/// What the first launch says lathe is, in one line.
const WHAT_LATHE_IS: &str = "Run coding agents on your code, review every change they make, and commit what you keep.";

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
    fn root(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // The side panes keep their width when the other hides; the agent panel takes what is left.
        let body = match self.active().cloned() {
            None => self.start_screen(window, cx).into_any_element(),
            Some(project) => self.panes(&project, window, cx),
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
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::open_ssh_form))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::new_session_key))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_right))
            .on_action(cx.listener(Self::pull_requests_key))
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
            .children(self.settings.as_ref().map(|(pane, _)| div().absolute().top(px(TITLE_BAR)).left_0().right_0().bottom_0().child(pane.clone())))
            // The dialogs share the Modal: a scrim, Escape and a press on the scrim close it, and focus goes back.
            .children(self.ssh.as_ref().map(|(form, _)| {
                let this = cx.entity().downgrade();
                let focus = form.read(cx).focus_handle(cx);
                Modal::new("open-over-ssh")
                    .view(form.read(cx).view_key())
                    .width(480.)
                    .focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_ssh(window, cx))))
                    .child(form.clone())
            }))
            .children(self.folder.as_ref().map(|(picker, _)| {
                let this = cx.entity().downgrade();
                let focus = picker.read(cx).focus_handle(cx);
                Modal::new("open-folder-picker")
                    .width(560.)
                    .focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_folder_picker(window, cx))))
                    .child(picker.clone())
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

/// Whose folders the folder picker lists.
#[derive(Clone)]
enum FolderSource {
    Local,
    /// A host's, through a project connected at its home folder.
    Remote { host: String, project: Arc<dyn lathe_project::Project> },
}

/// A failure to read or open a folder, as the picker tells it.
fn folder_error(error: &std::io::Error) -> beui::FolderError {
    match error.kind() {
        std::io::ErrorKind::NotFound => beui::FolderError::Missing,
        std::io::ErrorKind::PermissionDenied => beui::FolderError::Denied,
        std::io::ErrorKind::NotADirectory => beui::FolderError::NotAFolder,
        _ => beui::FolderError::Other(error.to_string().into()),
    }
}

/// A pane edge a reader drags to size the pane beside it.
#[derive(Clone, Copy)]
enum Edge {
    Sidebar,
    Right,
}

#[cfg(test)]
mod tests;

