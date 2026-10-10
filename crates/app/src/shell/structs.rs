use std::{path::PathBuf, rc::Rc, sync::Arc};
use std::collections::BTreeMap;

use atelier_ui::{
    PressStop,
    button::{Button, ButtonSize, ButtonVariant},
    file_icon::FileIcon,
    finder::{Filter, Finder, FinderEvent, FinderItem},
    keys::{self, Command, Press},
    modal::Modal,
    popover::{Hang, Popover},
    segmented::{Segment, Segmented},
    theme::{ActiveTheme, radius},
    typography::{FONT_FAMILY, TextSize},
};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, WindowControlArea, actions, div,
    prelude::FluentBuilder,
};
use atelier_ui::scale::px;
use atelier_project::LocalProject;
use atelier_settings::Location;
use atelier_ui::{
    agent_panels::AgentPanels,
    panel_types::{Layout, PanelsEvent, PanelsState},
    sidebar::{Sidebar, SidebarEvent},
};
use gpui_kit::AnyElement;

pub use super::view::{FilesPane, ShellView};
use super::fit::{Fit, Pane};
use crate::{
    agent_session::{self, AgentSession},
    agents_view,
    open_project::{Listing, OpenProject, ProjectEvent},
    review_pane::Scope,
    ssh_form::{Phase, SshForm, SshFormEvent},
    tree_view::tree_view,
};
use super::types::{
    AGENT_BESIDE_RIGHT, AGE_TICK, Edge, FolderSource, NOTICE_FOR, TITLE_BAR, TitleTabs, WIDE_RIGHT,
    TRAFFIC_LIGHTS, WHAT_ATELIER_IS,
};
use super::helpers::{folder_error, settings_path, tab_room};

actions!(atelier, [CheckForUpdates, ZoomIn, ZoomOut, ZoomReset, ShowSessions, OpenTasks, OpenFolder, OpenRemote, NewSession, Save, CloseTab, ToggleSidebar, ToggleRight, PullRequests, OpenSettings, Quit]);

pub struct Shell {
    pub(super) projects: Vec<Entity<OpenProject>>,
    pub(super) active: usize,
    pub(super) sidebar: bool,
    pub(super) right: bool,
    pub(super) recent: Vec<Location>,
    /// The last thing a project or the shell said: a notice at the foot of the window for a few seconds, and the start
    /// screen's error line.
    said: Option<SharedString>,
    /// The window's width as of the last frame.
    pub(super) width: f32,
    /// Go to file, while it is open: the finder and the paths its rows stand for.
    finder: Option<(Entity<Finder>, Vec<String>, Subscription)>,
    /// "Open over SSH…", while it is open.
    /// The Settings pane, while it is open.
    pub(super) settings: Option<(Entity<crate::settings_pane::SettingsPane>, Subscription)>,
    pub(super) ssh: Option<(Entity<SshForm>, Subscription)>,
    /// The in-app folder picker, when the system has none.
    pub(super) folder: Option<(Entity<atelier_ui::FolderPicker>, Subscription)>,
    pub(super) focus: FocusHandle,
    /// The projects and their sessions.
    pub(super) agents_sidebar: Entity<Sidebar>,
    /// The open sessions' panels.
    pub(super) panels: Entity<AgentPanels>,
    /// The single view's tabs, drawn in the title bar.
    tab_strip: Entity<atelier_ui::panel_tabs::TabStrip>,
    /// The open sessions' keys from the left of the strip: a new session stands first.
    pub(super) order: Vec<SharedString>,
    /// Names the reader gave sessions, by the agent's id.
    pub(super) names: BTreeMap<String, String>,
    /// The colours and images the reader gave projects' badges.
    pub(super) badges: agents_view::Badges,
    /// "Choose an icon…", while it is open: the chooser, the project's place, and its events.
    icon: Option<(Entity<atelier_ui::icon_picker::IconPicker>, SharedString, Subscription)>,
    /// With `ATELIER_FRAMES=1`, times every frame.
    meter: Option<Rc<std::cell::RefCell<crate::frame_meter::Meter>>>,
    /// The sidebar's and the right pane's widths as the reader dragged them; the window's width may
    /// show them narrower (`fit::widths`).
    pub(super) sidebar_width: f32,
    pub(super) right_width: f32,
    /// Where the bar at the foot starts, and the widths of its cards under the sidebar and the right pane: set as the panes are laid out.
    pub(super) footer_plan: (f32, Option<f32>, Option<f32>),
    /// Whether the sidebar shows in a window too narrow for it by default, after ⌘B.
    pub(super) sidebar_in_medium: bool,
    /// The pane a narrow window shows.
    pub(super) narrow: Pane,
    /// Sessions open at the last quit, waiting for their project to open.
    restoring: Vec<atelier_settings::OpenSession>,
    /// Projects being opened: until they all arrive, a saved session may still find its project.
    opening: usize,
    /// The session in front at the last quit, by the agent's id.
    pub(super) front: Option<String>,
    /// The open sessions and the one in front as the settings file has them, to write only a change.
    saved_open: (Vec<atelier_settings::OpenSession>, Option<String>),
    pub(super) _subscriptions: Vec<Subscription>,
    /// Each open session's panel view, by the session's entity.
    panel_views: std::collections::HashMap<gpui_kit::EntityId, Entity<crate::session_panel::SessionPanel>>,
    /// Which view shows: Sessions or Files.
    pub(super) view: ShellView,
    /// The view of the Code lens the rail goes back to.
    pub(super) code_view: ShellView,
    /// The Messages view's pane, made the first time the view is in front.
    pub(super) messages: Option<Entity<crate::messages::pane::MessagesPane>>,
    /// The Mail view's pane, made the first time the view is in front.
    pub(super) mail: Option<Entity<crate::mail::pane::MailPane>>,
    /// The Usage view's page, made the first time the view is in front.
    pub(super) usage: Option<Entity<crate::usage_view::UsagePage>>,
    /// A module asked for the Usage view: the next frame, which has a window, shows it.
    pub(super) usage_asked: bool,
    /// In Sessions, the project the list and the panels are narrowed to; all of them with `None`.
    pub(super) session_filter: Option<SharedString>,
    /// The project switcher's menu is open.
    pub(super) switcher_open: bool,
    /// The add button's menu next to the switcher.
    pub(super) add_open: bool,
    /// The file the History view shows of the picked commit, and the one Changes shows of the checkout: the tree's
    /// first while none is picked, or the picked one is not there.
    pub(super) history_file: Option<SharedString>,
    pub(super) change_file: Option<SharedString>,
    /// The height a file's diff had on the last frame, which its rows fill.
    pub(super) diff_height: f32,
    /// In a narrow window, the Files view's tree or editor.
    files_narrow: FilesPane,
    /// The ⋯ layout menu is open.
    layout_menu: bool,
    /// Where an update stands.
    pub(super) update: crate::updater::UpdateState,
    /// The changelog of the update this version came from, kept until the reader closes its sheet, and whether the sheet is open.
    pub(super) whats_new: Option<atelier_settings::WhatsNew>,
    pub(super) whats_new_open: bool,
    /// The changelog the version in the title bar opens.
    pub(super) changelog_open: bool,
    /// What the changelog panel holds focus with, so Escape reaches it.
    pub(super) update_focus: FocusHandle,
    /// The view a module opened over the window, by the id of its slot, until it closes itself.
    pub(super) opened: Option<(gpui_kit::SharedString, gpui_kit::AnyView)>,
    /// The sessions the reader archived, by the agent's id.
    pub(super) archived: std::collections::BTreeSet<String>,
    /// Where the session column ends, for the ⋯ at its top right; `None` in a narrow window.
    session_right: Option<f32>,
    /// The right pane's own view (`right_pane.rs`), cached.
    right_view: Entity<crate::right_pane::RightPane>,
    /// Gives the cached sidebar the time each minute, so a session's age moves on.
    _ages: gpui_kit::Task<()>,
    /// The numbers of the bar at the foot, and the loops that keep them.
    pub(super) vitals: Entity<crate::vitals::Vitals>,
    _vitals: Vec<gpui_kit::Task<()>>,
}

impl Shell {
    pub fn new(saved: &atelier_settings::Settings, cx: &mut Context<Self>) -> Self {
        let (vitals, _vitals) = Self::start_vitals(cx);
        // The first start after an update opens the changelog by itself, once: closing it forgets the kept record.
        let whats_new = Self::remembered_at_start(saved, cx);
        // The zoom the reader left it at.
        atelier_ui::scale::set_zoom(saved.ui_zoom.unwrap_or(1.));
        // Projects are added from the title bar, beside the switcher.
        let agents_sidebar = cx.new(|cx| {
            let mut sidebar = Sidebar::new(cx);
            sidebar.set_add_button(false, cx);
            sidebar
        });
        agents_sidebar.update(cx, |s, cx| s.set_layout(crate::sidebar_layout::from_settings(saved), cx));
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
            width: 1200.,
            finder: None,
            ssh: None,
            folder: None,
            settings: None,
            focus: cx.focus_handle(),
            agents_sidebar,
            tab_strip: cx.new(|cx| atelier_ui::panel_tabs::TabStrip::new(panels.clone(), cx)),
            panels,
            order: Vec::new(),
            names: saved.session_names.clone(),
            badges: agents_view::Badges::saved(saved),
            icon: None,
            meter: crate::frame_meter::enabled().then(Default::default),
            sidebar_width: super::fit::SIDEBAR_DEFAULT,
            right_width: super::fit::RIGHT_DEFAULT,
            footer_plan: (8., None, None),
            sidebar_in_medium: false,
            narrow: Pane::Session,
            restoring: Vec::new(),
            opening: 0,
            front: None,
            saved_open: (saved.open.clone(), saved.front.clone()),
            // An account connected, forgotten or rebuilt in Settings reaches the open Messages and Mail screens at once, as it
            // reaches Tasks.
            _subscriptions: vec![cx.observe_global::<crate::capability_hub::CapabilityHub>(|this, cx| {
                this.refresh_messages(cx);
                this.refresh_mail(cx);
            })],
            panel_views: Default::default(),
            view: ShellView::from_words(saved.view.as_deref()),
            files_narrow: FilesPane::default(),
            layout_menu: false,
            update: crate::updater::UpdateState::default(),
            whats_new_open: whats_new.is_some(),
            whats_new,
            changelog_open: false,
            update_focus: cx.focus_handle(),
            opened: None,
            code_view: Some(ShellView::from_words(saved.view.as_deref())).filter(|v| v.in_code()).unwrap_or(ShellView::Files),
            messages: None,
            mail: None,
            usage: None,
            usage_asked: false,
            session_filter: None,
            switcher_open: false,
            add_open: false,
            history_file: None,
            change_file: None,
            diff_height: 600.,
            archived: saved.archived_sessions.iter().cloned().collect(),
            session_right: None,
            right_view: cx.new(|_| crate::right_pane::RightPane::default()),
            vitals,
            _vitals,
            _ages: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(AGE_TICK).await;
                    if this.update(cx, |shell, cx| shell.tick_ages(cx)).is_err() {
                        break;
                    }
                }
            }),
        }
    }

    /// The sidebar's rows again with the time now, for their ages; nothing when no session row shows.
    fn tick_ages(&mut self, cx: &mut Context<Self>) {
        if !self.projects.iter().any(|p| !p.read(cx).sessions.is_empty()) {
            return;
        }
        self.push_sidebar(cx);
    }

    /// The sidebar's projects from the sessions as they are now, narrowed by the filter and the box.
    fn push_sidebar(&mut self, cx: &mut Context<Self>) {
        let all = agents_view::sidebar(&self.projects, &self.names, &self.badges, &self.archived, cx);
        // Each session carries its project's badge, for the head of its panel.
        let sessions: Vec<(Vec<Entity<AgentSession>>, atelier_ui::sidebar_model::Badge)> =
            self.projects.iter().zip(&all).map(|(p, data)| (p.read(cx).sessions.clone(), data.badge.clone())).collect();
        for (open, badge) in sessions {
            for session in open {
                session.update(cx, |s, cx| s.set_badge(badge.clone(), cx));
            }
        }
        let now = agent_session::now();
        let all = match &self.session_filter {
            Some(place) => all.into_iter().filter(|p| p.id == *place).collect(),
            None => all,
        };
        let handoff: Vec<_> = self.projects.iter().map(|p| (agents_view::project_id(p.read(cx)), p.read(cx).handoff_branches())).collect();
        self.agents_sidebar.update(cx, |s, cx| {
            for (project, targets) in handoff {
                s.set_handoff(project, targets, cx);
            }
            s.set_projects(all, now, cx);
        });
    }

    /// Hears the sidebar and the panels. Called once the window exists.
    pub fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = self.agents_sidebar.clone();
        let panels = self.panels.clone();
        self._subscriptions.push(cx.subscribe_in(&sidebar, window, Self::sidebar_event));
        self._subscriptions.push(cx.subscribe_in(&panels, window, Self::panels_event));
    }

    /// The sidebar and the panels, drawn again from the projects as they are now.
    pub(super) fn sync(&mut self, cx: &mut Context<Self>) {
        // Each open session keeps one view across syncs, so its panel is drawn from its last frame.
        let sessions: Vec<Entity<AgentSession>> = self.projects.iter().flat_map(|p| p.read(cx).sessions.clone()).collect();
        self.panel_views.retain(|id, _| sessions.iter().any(|s| s.entity_id() == *id));
        for session in &sessions {
            self.panel_views
                .entry(session.entity_id())
                .or_insert_with(|| cx.new(|cx| crate::session_panel::SessionPanel::new(session.clone(), cx)));
        }
        let views = &self.panel_views;
        let (panels, project_order) = agents_view::panels(&self.projects, &|s| views[&s.entity_id()].clone().into(), cx);
        let panels = agents_view::newest_first(panels, |p| &p.id, &mut self.order);
        let panels = match &self.session_filter {
            Some(place) => panels.into_iter().filter(|p| p.project.id == *place).collect(),
            None => panels,
        };
        self.push_sidebar(cx);
        self.panels.update(cx, |p, cx| p.set_panels(panels, project_order, cx));
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
                    open.push((s.key.clone(), atelier_settings::OpenSession {
                        location: p.location.clone(),
                        id: id.as_str().to_string(),
                        title: s.shown_title().to_string(),
                        agent: Some(s.agent.backend.name().to_string()),
                        provider: s.provider.as_ref().map(crate::providers::Choice::key),
                    }));
                }
            }
        }
        // From the left of the strip, so the next launch opens them in the same order.
        open.sort_by_key(|(key, _)| self.order.iter().position(|k| k == key));
        let open: Vec<_> = open.into_iter().map(|(_, session)| session).collect();
        let front = self.panels.read(cx).active().and_then(|key| self.session_by_key(key, cx)).and_then(|(_, s)| s.read(cx).id.clone()).map(|id| id.as_str().to_string());
        let now = (open, front);
        if now == self.saved_open {
            return;
        }
        self.saved_open = now.clone();
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| (s.open, s.front) = now) {
                    eprintln!("could not keep the open sessions: {error}");
                }
            })
            .detach();
        }
    }

    /// The sessions open at the last quit: each opens again when its project opens. With `open_projects`
    /// (no folder named at launch), their projects open too; else only the named ones do.
    pub fn restore(&mut self, open: Vec<atelier_settings::OpenSession>, front: Option<String>, open_projects: bool, window: &mut Window, cx: &mut Context<Self>) {
        let places = super::restore::locations(&open);
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
        let mine: Vec<atelier_settings::OpenSession> = super::restore::of(&self.restoring, &location).into_iter().cloned().collect();
        if mine.is_empty() {
            return;
        }
        self.restoring.retain(|s| s.location != location);
        if crate::timings::enabled() {
            window.on_next_frame(|_, _| {
                eprintln!("restored after {:.1} ms", crate::timings::since_start().as_secs_f64() * 1000.)
            });
        }
        let mut shown = None;
        for saved in mine {
            let id = atelier_agents::session::SessionId::new(saved.id.clone());
            let title = self.names.get(&saved.id).cloned().unwrap_or(saved.title.clone());
            let agent = saved.agent.as_deref().and_then(atelier_agents::registry::by_backend);
            let session = project.update(cx, |p, cx| p.open_session_on(Some((id, title.into())), agent, saved.provider.as_deref().and_then(crate::providers::Choice::from_key), window, cx));
            // A session from the last run keeps its place; only one opened now stands first.
            self.order.push(session.read(cx).key.clone());
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
    /// Tells the sidebar which session is the one in front. Only the single view has one: side by side, every panel
    /// is in front at once, so no row is marked.
    fn mark_open_session(&mut self, cx: &mut Context<Self>) {
        let panels = self.panels.read(cx);
        let front = (panels.layout() == Layout::Single).then(|| panels.active().cloned()).flatten();
        self.agents_sidebar.update(cx, |s, cx| s.set_open(front, cx));
    }
    fn project_by_id(&self, id: &str, cx: &App) -> Option<usize> {
        self.projects.iter().position(|p| agents_view::project_id(p.read(cx)).as_ref() == id)
    }

    /// The open session keyed `key`, and the index of its project.
    pub(super) fn session_by_key(&self, key: &str, cx: &App) -> Option<(usize, Entity<AgentSession>)> {
        self.projects.iter().enumerate().find_map(|(i, p)| {
            p.read(cx).sessions.iter().find(|s| s.read(cx).key.as_ref() == key).map(|s| (i, s.clone()))
        })
    }

    fn new_session(&mut self, project: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.projects.get(project).cloned() else { return };
        let session = p.update(cx, |p, cx| p.open_session(None, None, window, cx));
        self.show_session(project, &session, window, cx);
    }

    /// Opens a new session in the project at `project` that carries on the session on the sidebar's row `row`, on the agent
    /// and provider the id `target` names. It shows in front.
    fn handoff(&mut self, project: usize, row: &str, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.source_of(project, row, cx) else {
            return self.say("This session has not started, so there is nothing to hand off yet.".into(), cx);
        };
        let Some(p) = self.projects.get(project).cloned() else { return };
        let Some(target) = crate::handoff_targets::Target::parse(target) else {
            return self.say("This target is not known.".into(), cx);
        };
        match p.update(cx, |p, cx| p.handoff(source, &target, window, cx)) {
            Some(session) => self.show_session(project, &session, window, cx),
            None => self.say("This project does not offer that agent.".into(), cx),
        }
    }

    /// The session on the sidebar's row `row`, as a new session continues it; `None` before its agent named it.
    fn source_of(&self, project: usize, row: &str, cx: &App) -> Option<crate::agent_session::handoff::Source> {
        use crate::agent_session::handoff::Source;
        match agents_view::pick(row) {
            agents_view::Pick::Open(key) => {
                let (_, session) = self.session_by_key(&key, cx)?;
                let s = session.read(cx);
                Some(Source { backend: s.agent.backend.clone(), agent: s.agent.name.into(), id: s.id.clone()?, title: s.shown_title() })
            }
            agents_view::Pick::Past(id) => {
                let p = self.projects.get(project)?.read(cx);
                let title = self.names.get(&id.0).cloned().or_else(|| p.past.iter().find(|s| s.id == id).map(|s| s.title.clone())).unwrap_or_default();
                Some(Source { backend: p.agent.backend.clone(), agent: p.agent.name.into(), id, title: title.into() })
            }
        }
    }

    /// Makes `session` the panel in front, and its project the one the tree and the editor show.
    pub(super) fn show_session(&mut self, project: usize, session: &Entity<AgentSession>, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn sidebar_event(&mut self, _: &Entity<Sidebar>, event: &SidebarEvent, window: &mut Window, cx: &mut Context<Self>) {
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
                        let session = p.update(cx, |p, cx| p.open_session(Some((id.clone(), title.into())), None, window, cx));
                        if let Some(name) = self.names.get(&id.0) {
                            session.update(cx, |s, _| s.name = Some(name.clone().into()));
                        }
                        self.show_session(at, &session, window, cx);
                    }
                }
            }
            SidebarEvent::LayoutChanged(layout) => self.keep_sidebar_layout(*layout, cx),
            SidebarEvent::AddFolder => self.open_folder(&OpenFolder, window, cx),
            SidebarEvent::AddRemote => self.open_ssh_form(&OpenRemote, window, cx),
            SidebarEvent::Archive { session, archive, .. } => self.set_archived(session, *archive, cx),
            SidebarEvent::CopySessionId { session, .. } => match self.agent_id_of(session, cx) {
                Some(id) => {
                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(id));
                    self.say("Copied the session id.".into(), cx);
                }
                None => self.say("This session has not started, so it has no id yet.".into(), cx),
            },
            SidebarEvent::CloseSession { session, .. } => self.close_session(session.as_ref(), cx),
            SidebarEvent::Handoff { project, session, target } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.handoff(at, session, target, window, cx);
                }
            }
            SidebarEvent::NewSession { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.new_session(at, window, cx);
                }
            }
            // The project's menu is the way into the Files view: its tree and its editor.
            SidebarEvent::OpenFiles { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.active = at;
                    self.files_narrow = FilesPane::Tree;
                    self.show_view(ShellView::Files, window, cx);
                }
            }
            SidebarEvent::Tasks { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.active = at;
                    self.show_tasks(window, cx);
                }
            }
            SidebarEvent::Worktrees { project } => {
                if let Some(at) = self.project_by_id(project, cx) {
                    self.active = at;
                    self.projects[at].update(cx, |p, cx| p.read_worktrees(cx));
                    self.show_view(ShellView::Git, window, cx);
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
            SidebarEvent::ChooseIcon { project } => self.choose_icon(project.clone(), window, cx),
            // A remote project reconnects by itself; Retry says so.
            SidebarEvent::Retry { .. } => self.say("Reconnecting on its own; it retries every few seconds.".into(), cx),
        }
    }

    /// Closes the session keyed `key`: its panel goes, its agent stops, and it is in the project's past list.
    fn close_session(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some((at, _)) = self.session_by_key(key, cx) {
            self.projects[at].update(cx, |p, cx| p.close_session(key, cx));
            self.sync(cx);
        }
    }

    /// The id the agent knows the session on the sidebar's row `row` by: a past session's own, an open one's once it has started.
    fn agent_id_of(&self, row: &str, cx: &App) -> Option<String> {
        match agents_view::pick(row) {
            agents_view::Pick::Past(id) => Some(id.0),
            agents_view::Pick::Open(key) => self.session_by_key(&key, cx).and_then(|(_, s)| s.read(cx).id.as_ref().map(|i| i.as_str().to_string())),
        }
    }

    /// Puts the session on the sidebar's row `row` into the archive, or takes it out. An open session closes first:
    /// archiving puts it away. The choice is kept in the settings.
    pub(super) fn set_archived(&mut self, row: &str, archive: bool, cx: &mut Context<Self>) {
        let agent_id = self.agent_id_of(row, cx);
        let open = match agents_view::pick(row) {
            agents_view::Pick::Open(key) => Some(key),
            agents_view::Pick::Past(_) => None,
        };
        let Some(id) = agent_id else {
            return self.say("This session has not started, so it has nothing to archive yet.".into(), cx);
        };
        if archive {
            self.archived.insert(id.clone());
            if let Some(key) = open {
                self.close_session(&key, cx);
            }
        } else {
            self.archived.remove(&id);
        }
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                let result = atelier_settings::update(&path, |s| {
                    s.archived_sessions.retain(|kept| *kept != id);
                    if archive {
                        s.archived_sessions.push(id.clone());
                    }
                });
                if let Err(error) = result {
                    eprintln!("could not keep the archive: {error}");
                }
            })
            .detach();
        }
        self.sync(cx);
    }

    /// Zooms the interface to `to` (kept between the least and the most), keeps it for the next launch, and draws every view
    /// again: the cached ones are told, since they only draw again when notified.
    fn zoom_to(&mut self, to: f32, cx: &mut Context<Self>) {
        let kept = atelier_ui::scale::set_zoom(to);
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.ui_zoom = Some(kept)) {
                    eprintln!("could not keep the zoom: {error}");
                }
            })
            .detach();
        }
        self.agents_sidebar.update(cx, |_, cx| cx.notify());
        self.right_view.update(cx, |_, cx| cx.notify());
        self.panels.update(cx, |_, cx| cx.notify());
        for view in self.panel_views.values() {
            view.update(cx, |_, cx| cx.notify());
        }
        cx.refresh_windows();
        cx.notify();
    }

    /// Keeps what the sidebar's head chose that outlives the launch: how it lists (the filter starts as Active each
    /// time, so no session is hidden by a choice the reader forgot).
    fn keep_sidebar_layout(&mut self, layout: atelier_ui::sidebar_layout::SidebarLayout, cx: &mut Context<Self>) {
        let key = layout.mode.key().to_string();
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.sidebar = Some(key)) {
                    eprintln!("could not keep the sidebar's mode: {error}");
                }
            })
            .detach();
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
            PanelsEvent::Closed(key) => self.close_session(key.as_ref(), cx),
            PanelsEvent::StateChanged => {
                self.mark_open_session(cx);
                let state = self.panels.read(cx).state();
                let panels = atelier_settings::Panels {
                    single: state.layout == Layout::Single,
                    grouped: state.grouped,
                    widths: state.widths.iter().map(|(id, w)| (id.to_string(), *w)).collect(),
                };
                if let Some(path) = settings_path() {
                    cx.background_spawn(async move {
                        if let Err(error) = atelier_settings::update(&path, |s| s.panels = panels) {
                            eprintln!("could not save the panels: {error}");
                        }
                    })
                    .detach();
                }
            }
        }
    }

    pub(super) fn new_session_key(&mut self, _: &NewSession, window: &mut Window, cx: &mut Context<Self>) {
        if !self.projects.is_empty() {
            self.new_session(self.active, window, cx);
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub(super) fn active(&self) -> Option<&Entity<OpenProject>> {
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
                        let location = Location::Local { path: atelier_project::Project::root(&project).to_path_buf() };
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
    pub(super) fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.create_settings(window, cx);
    }

    /// The Settings pane on its Accounts section, from the Tasks screen's sign-in button: opened, or shown there when open
    /// already (unlike ⌘, it never closes the page).
    pub(super) fn open_accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = match &self.settings {
            Some((pane, _)) => pane.clone(),
            None => self.create_settings(window, cx),
        };
        pane.update(cx, |pane, cx| pane.show(crate::settings_pane::Section::Accounts, cx));
    }

    fn create_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<crate::settings_pane::SettingsPane> {
        let saved = settings_path().map(|p| atelier_settings::load(&p)).unwrap_or_default();
        let agents = atelier_agents::registry::agents()
            .into_iter()
            .map(|agent| crate::settings_pane::AgentRow {
                name: agent.name.into(),
                mark: agent.mark.clone(),
                backend: agent.backend.name().to_string().into(),
            })
            .collect();
        let pane = cx.new(|cx| crate::settings_pane::SettingsPane::new(&saved, agents, cx));
        // The repository of the project in front, for the Accounts section to offer for GitHub Issues.
        let github_repo = self.active().and_then(|p| p.read(cx).repo.clone()).filter(|r| r.host == "github.com").map(|r| r.slug());
        pane.update(cx, |pane, cx| pane.set_project_repo(github_repo, cx));
        let events = cx.subscribe_in(&pane, window, |this, _, event: &crate::settings_pane::SettingsEvent, window, cx| match event {
            crate::settings_pane::SettingsEvent::Close => {
                this.settings = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
            // The Settings page changed how the sidebar looks: it takes the look and keeps its own mode and filter.
            crate::settings_pane::SettingsEvent::Sidebar(look) => {
                let look = *look;
                this.agents_sidebar.update(cx, |s, cx| s.set_layout(s.layout().with_look_of(&look), cx));
            }
            crate::settings_pane::SettingsEvent::Zoom(zoom) => this.zoom_to(*zoom, cx),
        });
        pane.read(cx).focus_handle(cx).focus(window, cx);
        self.settings = Some((pane.clone(), events));
        cx.notify();
        pane
    }


    pub(super) fn open_ssh_form(&mut self, _: &OpenRemote, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.new(|cx| SshForm::new(Vec::new(), window, cx));
        // ~/.ssh/config is read off the UI thread; the form fills its hosts in when it has them.
        let reading = cx.background_spawn(async { atelier_remote::ssh::known_hosts() });
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

    /// Opens `path` on `host` over ssh: the host is probed and given atelier-remote if need be, all on
    /// a background thread, and the form, when open, shows each step and any failure.
    pub fn open_remote(&mut self, host: String, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.opening += 1;
        let (tx, mut steps) = futures_channel::mpsc::unbounded::<String>();
        let connecting = {
            let (host, path) = (host.clone(), path.clone());
            cx.background_spawn(async move { atelier_remote::ssh::connect(&host, &path, &|line| drop(tx.unbounded_send(line))) })
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

    /// `element`, timed as the part `name` under `ATELIER_FRAMES`.
    pub(super) fn part(&self, name: &'static str, element: AnyElement) -> AnyElement {
        match self.meter.clone() {
            Some(meter) => crate::frame_meter::Part { name, child: element, meter }.into_any_element(),
            None => element,
        }
    }

    fn open_tasks_key(&mut self, _: &OpenTasks, window: &mut Window, cx: &mut Context<Self>) {
        self.show_tasks(window, cx);
    }
    /// Shows the Tasks view, with the active project's board; from the Tasks view, goes back to Sessions.
    pub(super) fn show_tasks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == ShellView::Tasks {
            return self.show_view(ShellView::Sessions, window, cx);
        }
        let Some(project) = self.active().cloned() else { return };
        self.show_view(ShellView::Tasks, window, cx);
        project.update(cx, |p, cx| p.mount_tasks(window, cx));
        if let Some(tasks) = &project.read(cx).tasks {
            tasks.pane.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    fn pull_requests_key(&mut self, _: &PullRequests, window: &mut Window, cx: &mut Context<Self>) {
        self.show_pulls(window, cx);
    }

    /// Shows or hides the active project's pull requests in the right pane, wide.
    fn show_pulls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.active().cloned() else { return };
        self.view = ShellView::Sessions;
        project.update(cx, |p, cx| p.toggle_pulls(window, cx));
        self.right = true;
        self.widen_right(window, cx);
        self.focus_front(&project, window, cx);
        cx.notify();
    }

    /// The keys go to what is in front now: the review when one is open, else the tasks when the right
    /// pane shows them, else the shell, so no key is left with a pane that is no longer drawn.
    fn focus_front(&mut self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) {
        let p = project.read(cx);
        match (p.front(), p.review.as_ref()) {
            (_, Some((pane, _))) => pane.focus_handle(cx).focus(window, cx),
            (crate::open_project::front::Front::Tasks, _) => match &p.tasks {
                Some(tasks) => tasks.pane.focus_handle(cx).focus(window, cx),
                None => self.focus.focus(window, cx),
            },
            _ => self.focus.focus(window, cx),
        }
    }

    /// Gives the right pane the width the tasks and the pull requests want, taken from the agent panel while
    /// a session panel still fits in it.
    fn widen_right(&mut self, window: &mut Window, _: &mut Context<Self>) {
        let total = atelier_ui::scale::design(window.viewport_size().width);
        let fit = Fit::of(total);
        if fit == Fit::Narrow {
            self.narrow = Pane::Right;
            return;
        }
        let sidebar = if self.sidebar_shown(fit) { self.sidebar_width } else { 0. };
        let want = WIDE_RIGHT.min(total - sidebar - AGENT_BESIDE_RIGHT);
        if self.right_width < want {
            self.right_width = want;
        }
    }

    /// Whether the sidebar shows at `fit`: by the reader's choice in a wide window, after ⌘B in a
    /// medium one, and as its own tab in a narrow one.
    pub(super) fn sidebar_shown(&self, fit: Fit) -> bool {
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
        self.show_file(cx);
    }

    /// Shows `view`. The way into Files is a project's menu; ⌘1 (⌃ elsewhere) goes back to Sessions. The focus comes to the shell: what had it (a
    /// composer, the editor) is not drawn in the other view, and a key from it would reach nothing.
    pub fn show_view(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        if view.in_code() {
            self.code_view = view;
        }
        if self.view != view {
            self.view = view;
            self.save_view(cx);
            // Back in Sessions the reader is in the composer of the session in front, so typing goes on there.
            let composer = (view == ShellView::Sessions)
                .then(|| self.panels.read(cx).active().and_then(|key| self.session_by_key(key, cx)))
                .flatten()
                .map(|(_, session)| session.read(cx).composer.clone());
            match composer {
                Some(composer) => composer.read(cx).focus_handle(cx).focus(window, cx),
                None => self.focus.focus(window, cx),
            }
            cx.notify();
        }
    }

    /// Keeps the view for the next launch, off the UI thread.
    pub(super) fn save_view(&self, cx: &mut Context<Self>) {
        let words = self.view.words().to_string();
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.view = Some(words)) {
                    eprintln!("could not keep the view: {error}");
                }
            })
            .detach();
        }
    }

    /// A file was opened: the Files view shows it, in a narrow window with the editor in front.
    fn show_file(&mut self, cx: &mut Context<Self>) {
        self.files_narrow = FilesPane::Editor;
        if self.view != ShellView::Files {
            self.view = ShellView::Files;
            self.save_view(cx);
        }
        cx.notify();
    }

    pub(super) fn add(&mut self, location: Location, project: Arc<dyn atelier_project::Project>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.projects.iter().position(|p| p.read(cx).location == location) {
            self.active = i;
            cx.notify();
            return;
        }
        self.said = None;
        let entity = cx.new(|cx| OpenProject::new(location.clone(), project, window, cx));
        self._subscriptions.push(cx.subscribe_in(&entity, window, |this, project, event: &ProjectEvent, window, cx| match event {
            ProjectEvent::Said(line) => this.say(line.to_string(), cx),
            ProjectEvent::Open(path) => this.open_in(project, path, window, cx),
            ProjectEvent::Review { session, turn, path } => {
                let scope = turn.map_or(Scope::Whole, Scope::Turn);
                this.review(project, session.clone(), scope, path.as_deref(), window, cx);
            }
            ProjectEvent::ShowSession(session) => {
                if let Some(at) = this.projects.iter().position(|p| p == project) {
                    this.show_session(at, session, window, cx);
                }
            }
            ProjectEvent::TasksShown => {
                this.right = true;
                this.widen_right(window, cx);
                this.focus_front(project, window, cx);
                cx.notify();
            }
            ProjectEvent::PullPage => this.show_code(ShellView::Pulls, window, cx),
            ProjectEvent::PullsShown => {
                this.right = true;
                this.widen_right(window, cx);
                this.focus_front(project, window, cx);
                cx.notify();
            }
            ProjectEvent::ReviewClosed => {
                // The review's texts and hunks are freed now: give their pages back.
                crate::memory::give_back();
                if this.view == ShellView::Git {
                    this.show_view(ShellView::Sessions, window, cx);
                }
                cx.notify();
            }
            ProjectEvent::CloseSession(key) => this.close_session(key.as_ref(), cx),
            ProjectEvent::NewSessionHere => {
                if let Some(at) = this.projects.iter().position(|p| p == project) {
                    this.new_session(at, window, cx);
                }
            }
            ProjectEvent::ArchiveSession(key) => this.set_archived(key.as_ref(), true, cx),
            ProjectEvent::Handoff { session, target } => {
                if let Some(at) = this.projects.iter().position(|p| p == project) {
                    this.handoff(at, session.as_ref(), target.as_ref(), window, cx);
                }
            }
            ProjectEvent::ShowFiles => {
                if let Some(at) = this.projects.iter().position(|p| p == project) {
                    this.active = at;
                }
                this.show_view(ShellView::Files, window, cx)
            }
            ProjectEvent::OpenAccounts => this.open_accounts(window, cx),
            ProjectEvent::ShowTasks => {
                if let Some(i) = this.projects.iter().position(|p| p == project) {
                    this.active = i;
                }
                this.show_tasks(window, cx);
            }
            ProjectEvent::Sessions => this.sync(cx),
            ProjectEvent::Renamed { id, name } => {
                this.names.insert(id.0.clone(), name.to_string());
                let (id, name) = (id.0.clone(), name.to_string());
                if let Some(path) = settings_path() {
                    cx.background_spawn(async move {
                        if let Err(error) = atelier_settings::update(&path, |s| drop(s.session_names.insert(id, name))) {
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
        let mut settings = atelier_settings::Settings { recent: std::mem::take(&mut self.recent), ..Default::default() };
        settings.opened(location.clone());
        self.recent = settings.recent;
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.opened(location)) {
                    eprintln!("could not remember the project: {error}");
                }
            })
            .detach();
        }
    }

    pub(super) fn say(&mut self, line: String, cx: &mut Context<Self>) {
        let line: SharedString = line.into();
        self.said = Some(line.clone());
        // On the start screen the line is the only answer to what was asked, such as a folder that is not there:
        // it stays until a project opens.
        if self.projects.is_empty() {
            return cx.notify();
        }
        // The notice goes after a few seconds, unless something newer was said in the meantime.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_FOR).await;
            _ = this.update(cx, |this, cx| {
                if this.said.as_ref() == Some(&line) {
                    this.said = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
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
    /// editor too; here, Go to file and Go to tasks (`g t`), and only while nothing is being typed.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let press = Press::from_keystroke(&event.keystroke);
        if press.secondary || keys::typing(window) {
            return;
        }
        match keys::read_now(&press, cx) {
            Some(Command::GoToFile) => {
                cx.stop_propagation();
                self.go_to_file(window, cx);
            }
            Some(Command::GoToTasks) => {
                cx.stop_propagation();
                self.show_tasks(window, cx);
            }
            _ => {}
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
                    this.show_file(cx);
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
        let picker = cx.new(|cx| atelier_ui::FolderPicker::new("~/", window, cx).with_recent(recent));
        let events = cx.subscribe_in(&picker, window, move |this, picker, event: &atelier_ui::FolderPickerEvent, window, cx| match event {
            atelier_ui::FolderPickerEvent::Want(dir) => {
                let dir = dir.to_string();
                let listing = cx.background_spawn({
                    let (dir, source) = (dir.clone(), source.clone());
                    async move {
                        match source {
                            FolderSource::Local => atelier_project::read_local_dir(&dir),
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
            atelier_ui::FolderPickerEvent::Choose(path) => {
                // The picker stays until the folder opens: a folder that will not open is said in the picker, with
                // the path as the reader typed it.
                match &source {
                    FolderSource::Local => match atelier_project::expand_home(path) {
                        Some(target) => {
                            let opening = cx.background_spawn(async move { LocalProject::open(target) });
                            let picker = picker.downgrade();
                            cx.spawn_in(window, async move |this, cx| {
                                let opened = opening.await;
                                _ = this.update_in(cx, |this, window, cx| match opened {
                                    Ok(project) => {
                                        this.close_folder_picker(window, cx);
                                        let location = Location::Local { path: atelier_project::Project::root(&project).to_path_buf() };
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
                            picker.update(cx, |p, cx| p.refuse(atelier_ui::FolderError::Missing, cx));
                        }
                    },
                    FolderSource::Remote { host, .. } => {
                        let (host, path) = (host.clone(), path.to_string());
                        picker.update(cx, |p, cx| p.working(Some(format!("Opening {path} on {host}…").into()), cx));
                        let connecting = {
                            let (host, path) = (host.clone(), path.clone());
                            cx.background_spawn(async move { atelier_remote::ssh::connect(&host, &path, &|_| {}) })
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
            atelier_ui::FolderPickerEvent::Cancel => this.close_folder_picker(window, cx),
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
            cx.background_spawn(async move { atelier_remote::ssh::connect_at_home(&host, &|line| drop(tx.unbounded_send(line))) })
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
                    let project: Arc<dyn atelier_project::Project> = Arc::new(project);
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

    /// "Choose an icon…": lists the image files of the project and lets the reader pick one for its badge.
    fn choose_icon(&mut self, place: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.project_by_id(&place, cx) else { return };
        let project = self.projects[at].clone();
        let Listing::Ready(tree) = &project.read(cx).listing else {
            self.say("The project is still being read. Try again in a moment.".into(), cx);
            return;
        };
        let paths = tree.file_paths();
        let root = match &project.read(cx).location {
            atelier_settings::Location::Local { path } => Some(path.clone()),
            atelier_settings::Location::Ssh { .. } => None,
        };
        let current = atelier_ui::project_badge::color_of(self.badges.colors.get(place.as_ref()).map(|c| usize::from(*c)), place.as_ref());
        let picker = cx.new(|cx| atelier_ui::icon_picker::IconPicker::new(paths, root, window, cx).with_color(current));
        let key = place.clone();
        let events = cx.subscribe_in(&picker, window, move |this, _, event: &atelier_ui::icon_picker::IconPickerEvent, window, cx| match event {
            atelier_ui::icon_picker::IconPickerEvent::Choose(path) => this.save_icon(key.clone(), Some(path.to_string()), window, cx),
            atelier_ui::icon_picker::IconPickerEvent::Color(index) => this.save_color(key.clone(), *index, cx),
            atelier_ui::icon_picker::IconPickerEvent::Clear => this.save_icon(key.clone(), None, window, cx),
            atelier_ui::icon_picker::IconPickerEvent::Cancel => this.close_icon_picker(window, cx),
        });
        picker.read(cx).focus_handle(cx).focus(window, cx);
        self.icon = Some((picker, place, events));
        cx.notify();
    }

    fn close_icon_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.icon.take().is_some() {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Keeps the colour the reader chose for the project's letter badge, a place in the palette. The chooser stays open:
    /// the colour is there to see on the badge behind it, and the image is still to choose.
    pub(super) fn save_color(&mut self, place: SharedString, index: usize, cx: &mut Context<Self>) {
        let key = place.to_string();
        let Ok(index) = u8::try_from(index) else { return };
        self.badges.colors.insert(key.clone(), index);
        if let Some(settings) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&settings, |s| {
                    s.project_colors.insert(key, index);
                }) {
                    eprintln!("could not save the colour: {error}");
                }
            })
            .detach();
        }
        self.sync(cx);
    }

    /// Keeps the chosen image for the project's badge: its bytes are copied into the data folder, since the file may be
    /// on another host, and the copy's path is saved. `None` puts the letter back.
    fn save_icon(&mut self, place: SharedString, file: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.close_icon_picker(window, cx);
        let Some(settings) = settings_path() else { return };
        let key = place.to_string();
        let Some(file) = file else {
            self.badges.icons.remove(&key);
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&settings, |s| drop(s.project_icons.remove(&key))) {
                    eprintln!("could not save the icon: {error}");
                }
            })
            .detach();
            self.sync(cx);
            return;
        };
        let Some(at) = self.project_by_id(&place, cx) else { return };
        let read = self.projects[at].update(cx, |p, cx| p.read_bytes(&file, cx));
        cx.spawn_in(window, async move |this, cx| {
            let bytes = match read.await {
                Ok(bytes) => bytes,
                Err(error) => {
                    _ = this.update(cx, |this, cx| this.say(format!("Could not read {file}: {error}"), cx));
                    return;
                }
            };
            let Some(folder) = settings.parent().map(|p| p.join("project-icons")) else { return };
            let copy = folder.join(crate::project_icons::file_name(&key, &file));
            let saved = copy.clone();
            let written = cx
                .background_spawn(async move {
                    std::fs::create_dir_all(&folder)?;
                    std::fs::write(&saved, bytes)?;
                    let path = saved.display().to_string();
                    atelier_settings::update(&settings, |s| drop(s.project_icons.insert(key, path))).map(drop)
                })
                .await;
            _ = this.update(cx, |this, cx| match written {
                Ok(()) => {
                    this.badges.icons.insert(place.to_string(), copy.display().to_string());
                    this.sync(cx);
                }
                Err(error) => this.say(format!("Could not keep the icon: {error}"), cx),
            });
        })
        .detach();
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
        self.flip_sidebar(Fit::of(atelier_ui::scale::design(window.viewport_size().width)), cx);
    }

    pub(super) fn flip_sidebar(&mut self, fit: Fit, cx: &mut Context<Self>) {
        match fit {
            Fit::Wide => self.sidebar = !self.sidebar,
            Fit::Medium => self.sidebar_in_medium = !self.sidebar_in_medium,
            Fit::Narrow => self.narrow = if self.narrow == Pane::Projects { Pane::Session } else { Pane::Projects },
        }
        cx.notify();
    }
    fn toggle_right(&mut self, _: &ToggleRight, window: &mut Window, cx: &mut Context<Self>) {
        match Fit::of(atelier_ui::scale::design(window.viewport_size().width)) {
            Fit::Narrow => self.narrow = if self.narrow == Pane::Right { Pane::Session } else { Pane::Right },
            Fit::Medium | Fit::Wide => self.right = !self.right,
        }
        cx.notify();
    }

    /// What the title bar's free room carries: the single view's session tabs in the Sessions view, the open files' tabs
    /// in the Files view. A narrow window has a row of tabs for its panes already, and Settings has none.
    fn title_tabs(&self, cx: &App) -> TitleTabs {
        if self.settings.is_some() || super::fit::Fit::of(self.width) == Fit::Narrow || self.active().is_none() {
            return TitleTabs::None;
        }
        match self.view {
            ShellView::Files => TitleTabs::Files,
            ShellView::Sessions
                if self.panels.read(cx).layout() == Layout::Single && self.projects.iter().any(|p| !p.read(cx).sessions.is_empty()) =>
            {
                TitleTabs::Sessions
            }
            _ => TitleTabs::None,
        }
    }

    fn title_bar(&self, tabs: TitleTabs, cx: &mut Context<Self>) -> impl IntoElement {
        let room = match tabs {
            TitleTabs::None => div().flex_1(),
            TitleTabs::Sessions => div()
                .debug_selector(|| "title-tabs".into())
                .flex_1()
                .min_w_0()
                .h_full()
                // The window's own line takes the bar's top pixel on a Mac: the tabs centre in what is left, so the room above
                // them (line to tab) is the room below them (tab to panes).
                .when(cfg!(target_os = "macos"), |d| d.pt(px(1.)))
                .mr(px(tab_room(self.width, self.session_right, self.update_chip_room())))
                .child(self.tab_strip.clone()),
            TitleTabs::Files => match self.active() {
                Some(project) => div()
                    .debug_selector(|| "title-tabs".into())
                    .flex_1()
                    .min_w_0()
                    .child(crate::editor_pane::editor_tabs(project, cx)),
                None => div().flex_1(),
            },
        };
        div()
            .id("title-bar")
            .window_control_area(WindowControlArea::Drag)
            // A double press on the bar zooms the window, as on every Mac window.
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.zoom_window();
                }
            })
            .flex()
            .flex_none()
            .items_center()
            .gap(px(super::types::PANE_GAP))
            // Scaled with the zoom, like what it holds: a tab 1.5 times taller needs a bar 1.5 times taller. Only the traffic lights beside it are native.
            .h(px(TITLE_BAR))
            .pl(gpui_kit::px(TRAFFIC_LIGHTS))
            .pr(px(12.))
            .text_size(TextSize::Sm.font_size())
            // No project is "the" project of the window: the title bar names the app, and each project names itself
            // in the sidebar, on its panels, and at the head of its Files.
            .child(self.title_left(cx))
            .relative()
            .children(self.layout_button(cx))
            .child(room)
            .children(self.update_chip(cx).map(|chip| div().flex_none().mr(px(4.)).child(chip)))
            .child(self.settings_button(cx))
    }

    /// What the title bar holds at its left: Back while Settings is open, "Sessions" back from the Files view, else the
    /// app's name.
    fn title_left(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        if self.settings.is_some() {
            return Button::new("settings-back")
                .debug_name("settings-back")
                .icon(atelier_ui::IconName::ArrowBack)
                .label("Back")
                .cap("Esc")
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, window, cx| this.update(cx, |this, cx| this.close_settings(window, cx)))
                .into_any_element();
        }
        self.sidebar_toggle(cx)
    }

    /// The sidebar's toggle at the left of the title bar, lit while the sidebar shows.
    fn sidebar_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        atelier_ui::view_rail::RailButton::new("sidebar-toggle", atelier_ui::IconName::SidebarLeft, "Toggle the sidebar (⌘b)")
            .debug_name("sidebar-toggle")
            .on_click(move |window, cx| {
                let fit = Fit::of(atelier_ui::scale::design(window.viewport_size().width));
                this.update(cx, |this, cx| this.flip_sidebar(fit, cx))
            })
            .into_any_element()
    }

    /// The Settings button at the top right, lit while the page is open.
    fn settings_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        let button = Button::new("settings-entry")
            .debug_name("settings-entry")
            .icon(atelier_ui::IconName::Settings)
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconSm)
            .tooltip("Settings")
            .open(self.settings.is_some())
            .on_click(move |_, window, cx| this.update(cx, |this, cx| this.open_settings(&OpenSettings, window, cx)));
        crate::control::marked("settings-entry", button)
    }

    /// The section of the Settings page that is open, by the name of its entry, or none when the page is closed.
    pub fn settings_section(&self, cx: &App) -> Option<&'static str> {
        self.settings.as_ref().map(|(pane, _)| pane.read(cx).section().entry())
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// The ⋯ at the top right of the session area, and its layout menu: side by side or single, grouped by
    /// project or not, each with its key. `None` while no session is open.
    fn layout_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use atelier_ui::{
            agent_panels::chord,
            menu::{self, Choice, Entry, Menu, MenuItem, MenuLook, Origin},
            panel_types::Layout,
            popover::{Hang, Popover},
        };
        if self.settings.is_some() || self.view != ShellView::Sessions || !self.projects.iter().any(|p| !p.read(cx).sessions.is_empty()) {
            return None;
        }
        let (layout, grouped) = {
            let panels = self.panels.read(cx);
            (panels.layout(), panels.grouped())
        };
        let this = cx.entity().downgrade();
        let menu = self.layout_menu.then(|| {
            let (side, single, group, close) = (this.clone(), this.clone(), this.clone(), this.clone());
            let entries: Vec<Entry> = vec![
                Entry::from(
                    MenuItem::new("Side by side")
                        .debug_name("layout-side-by-side")
                        .choice(Choice::Radio(layout == Layout::SideBySide))
                        .cap(keys::cap(chord::TOGGLE_LAYOUT))
                        .on_select(move |_, cx| drop(side.update(cx, |s, cx| s.choose_layout(Layout::SideBySide, cx)))),
                ),
                Entry::from(
                    MenuItem::new("Single view")
                        .debug_name("layout-single")
                        .choice(Choice::Radio(layout == Layout::Single))
                        .on_select(move |_, cx| drop(single.update(cx, |s, cx| s.choose_layout(Layout::Single, cx)))),
                ),
                Entry::from(
                    MenuItem::new("Group by project")
                        .debug_name("layout-grouped")
                        .choice(Choice::Check(grouped))
                        .cap(keys::cap(chord::TOGGLE_GROUPING))
                        .on_select(move |_, cx| drop(group.update(cx, |s, cx| s.choose_grouping(!grouped, cx)))),
                ),
            ];
            Popover::new("layout-menu-popover")
                .open(true)
                .hang(Hang::Right(0., 30.))
                .keep_focus()
                .height(menu::height_in(MenuLook::PROJECT, 3))
                .on_close(move |_, cx| drop(close.update(cx, |s, cx| s.close_layout_menu(cx))))
                .child(Menu::new("layout-menu-panel", entries).look(MenuLook::PROJECT).origin(Origin::TopRight))
        });
        let toggle = this.clone();
        let button = Button::new("layout-menu")
            .debug_name("layout-menu")
            .icon(atelier_ui::IconName::MoreHoriz)
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconSm)
            .tooltip("Layout")
            .open(self.layout_menu)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                drop(toggle.update(cx, |s, cx| {
                    s.layout_menu = !s.layout_menu;
                    cx.notify();
                }))
            });
        // The Settings button holds the window's top right corner; the layout menu stands left of it.
        let at = div().absolute().top(px(7.));
        let at = match self.session_right {
            Some(right) => at.left(px((right - 36.).min(self.width - 72. - self.update_chip_room()))),
            None => at.right(px(48. + self.update_chip_room())),
        };
        Some(at.child(div().relative().child(button).children(menu)).into_any_element())
    }

    pub(super) fn choose_layout(&mut self, layout: atelier_ui::panel_types::Layout, cx: &mut Context<Self>) {
        self.panels.update(cx, |p, cx| p.set_layout(layout, cx));
        self.mark_open_session(cx);
        self.close_layout_menu(cx);
    }

    fn choose_grouping(&mut self, grouped: bool, cx: &mut Context<Self>) {
        self.panels.update(cx, |p, cx| p.set_grouped(grouped, cx));
        self.close_layout_menu(cx);
    }

    fn close_layout_menu(&mut self, cx: &mut Context<Self>) {
        self.layout_menu = false;
        cx.notify();
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
                .gap(px(8.))
                .h(px(40.))
                .px(px(12.))
                .rounded(radius::lg())
                .cursor_pointer()
                .hover(|s| s.bg(theme.muted_hover()))
                .press_stop(gpui_kit::ElementId::Name(format!("recent-focus-{i}").into()), radius::lg(), window, cx)
                .on_click(cx.listener(move |this, _, window, cx| match &open {
                    Location::Local { path } => this.open_local(path.clone(), window, cx),
                    Location::Ssh { host, path } => this.open_remote(host.clone(), path.display().to_string(), window, cx),
                }))
                .child(match location {
                    Location::Ssh { .. } => atelier_ui::Icon::new(atelier_ui::IconName::Dns).size(px(16.)).color(muted).into_any_element(),
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
                    .child(div().flex().child(atelier_ui::AtelierMark::new(40.)))
                    .child(div().text_size(TextSize::Lg.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Open a project"))
                    .child(div().debug_selector(|| "first-launch-line".into()).mt(px(-12.)).text_size(TextSize::Sm.font_size()).text_color(muted).child(WHAT_ATELIER_IS))
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
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        // The project switcher heads every sidebar: which project the lens is about.
        let switcher = self.project_switcher(cx);
        // The Sessions sidebar has a row of its own at the top, with the ⋯ at its right: the switcher stands on that row,
        // left of the ⋯, so the head is one row. The other lenses have no such row and give the switcher one.
        if !self.view.in_code() && !matches!(self.view, ShellView::Tasks | ShellView::Messages | ShellView::Mail | ShellView::Usage) {
            let row = switcher.map(|switcher| {
                div().absolute().top(px(5.)).left(px(8.)).right(px(44.)).h(px(36.)).flex().items_center().min_w_0().child(switcher)
            });
            return div().relative().size_full().child(self.sidebar_body(cx)).children(row).into_any_element();
        }
        let head = switcher.map(|switcher| div().flex().flex_none().items_center().min_w_0().px(px(8.)).pt(px(8.)).pb(px(4.)).child(switcher));
        div().flex().flex_col().size_full().children(head).child(div().flex_1().min_h_0().child(self.sidebar_body(cx))).into_any_element()
    }
    fn sidebar_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // The projects and their sessions, or in the Git view the focused session's changes; the files
        // are the Files view's.
        if self.view.in_code()
            && let Some(project) = self.active().cloned()
        {
            return div().size_full().child(self.code_sidebar(&project, cx));
        }
        if self.view == ShellView::Tasks
            && let Some(project) = self.active().cloned()
        {
            return div().size_full().child(self.issues_sidebar(&project, cx));
        }
        if self.view == ShellView::Messages {
            return div().size_full().child(self.messages_sidebar(cx));
        }
        if self.view == ShellView::Mail {
            return div().size_full().child(self.mail_sidebar(cx));
        }
        if self.view == ShellView::Usage {
            return div().size_full().child(self.usage_sidebar(cx));
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().flex_1().min_h_0().child(crate::view_cache::draw(&self.agents_sidebar)))
    }

    /// The Files view's tree: the front project's files, under their heading.
    pub(super) fn files_tree(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let menu = project.clone();
        let ends_name = project.clone();
        div()
            .debug_selector(|| "files-tree".into())
            // A press anywhere in the tree ends a name being typed; the name's own row keeps its presses.
            .on_mouse_down(gpui_kit::MouseButton::Left, move |_, _, cx| ends_name.update(cx, |p, cx| p.cancel_tree_edit(cx)))
            // A press with the other button where there is no row opens the project's folder's menu.
            .on_mouse_down(gpui_kit::MouseButton::Right, move |event, _, cx| {
                let at = event.position;
                menu.update(cx, |p, cx| p.open_tree_menu(String::new(), true, at, cx));
            })
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .px(px(12.))
                    .pt(px(12.))
                    .pb(px(4.))
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(format!("Files in {}", project.read(cx).name())),
            )
            .child(div().flex_1().min_h_0().child(tree_view(project, cx)))
            .into_any_element()
    }

    /// The Files view's editor, on its card; it says "No file open" until a file is.
    fn files_editor(&self, project: &Entity<OpenProject>, tabs_above: bool, cx: &mut Context<Self>) -> AnyElement {
        let editor = if tabs_above {
            crate::editor_pane::editor_pane(project, cx).into_any_element()
        } else {
            crate::editor_pane::editor_below_tabs(project, cx).into_any_element()
        };
        self.code_card(div().size_full().pt(px(8.)).child(editor).into_any_element(), cx)
    }

    /// The Files view in a narrow window: the tree or the editor, with a tab for each.
    fn narrow_files(&mut self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        let panes = [FilesPane::Tree, FilesPane::Editor];
        let tabs = div().flex().flex_none().items_center().px(px(8.)).h(px(44.)).child(
            Segmented::new(
                "narrow-files",
                [Segment::new("Files").debug_name("narrow-tab-Files"), Segment::new("Editor").debug_name("narrow-tab-Editor")],
                panes.iter().position(|p| *p == self.files_narrow).unwrap_or(0),
            )
            .on_change(move |i, _, cx| {
                this.update(cx, |this, cx| {
                    this.files_narrow = panes[i];
                    cx.notify();
                })
            }),
        );
        let body = match self.files_narrow {
            FilesPane::Tree => self.files_tree(project, cx),
            FilesPane::Editor => self.files_editor(project, true, cx),
        };
        div()
            .debug_selector(|| "files-view".into())
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(tabs)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
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
    pub(super) fn panes(&mut self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // A window that closed on Messages opens on it with no press, so the pane is made here and not only by the rail.
        if self.view == ShellView::Messages {
            self.ensure_messages(window, cx);
        }
        if self.usage_asked {
            self.usage_asked = false;
            self.show_usage(window, cx);
        }
        if self.view == ShellView::Mail {
            self.ensure_mail(window, cx);
        }
        if self.view == ShellView::Usage {
            self.ensure_usage(cx);
        }
        let total = atelier_ui::scale::design(window.viewport_size().width);
        let fit = Fit::of(total);
        if fit == Fit::Narrow {
            self.session_right = None;
            return match self.view {
                ShellView::Files => self.narrow_files(project, cx),
                _ => self.narrow_panes(project, window, cx),
            };
        }
        // In the Sessions view the right pane holds the pull requests or the tasks; the editor is the
        // Files view's.
        let asked = self.view == ShellView::Sessions && project.read(cx).front() != crate::open_project::front::Front::Editor;
        let wants = super::fit::Wants {
            sidebar: self.sidebar_shown(fit).then_some(self.sidebar_width),
            right: (self.right && asked).then_some(self.right_width),
        };
        let rail = atelier_ui::view_rail::WIDTH;
        let widths = super::fit::widths(total - rail, wants);
        self.session_right = Some(rail + widths.sidebar.unwrap_or(0.) + widths.agent);
        // The strip lays its columns out from this width in this frame; the strip keeps 8 px each side.
        // The strip pads its sides by 8; next to the sidebar's card the left pad is the panels' own gap.
        let inset = super::types::PANE_GAP;
        // The bar's cards stand under the columns: the sidebar's, the session's and the right pane's, a panels' gap below them.
        self.footer_plan = (
            rail + super::types::PANE_GAP,
            widths.sidebar.map(|w| w - super::types::PANE_GAP),
            widths.right.map(|w| w - 8. - super::types::PANE_GAP),
        );
        let (lead, tail) = (self.footer_plan.1, self.footer_plan.2);
        self.vitals.update(cx, |v, cx| {
            if v.set_columns(lead, tail) {
                cx.notify();
            }
        });
        self.panels.update(cx, |p, cx| {
            p.set_inset_left(inset, cx);
            p.set_inset_bottom(0., cx);
            p.fit_to(widths.agent - inset - 8., cx)
        });
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
            d.debug_selector(match edge {
                Edge::Sidebar => || "edge-sidebar".into(),
                Edge::Right => || "edge-right".into(),
            })
            .absolute()
                .top_0()
                .bottom_0()
                .w(px(8.))
                .cursor(gpui_kit::CursorStyle::ResizeLeftRight)
                .hover(|d| d.bg(wash))
                .on_drag(edge, |_, _, _, cx| cx.new(|_| gpui_kit::Empty))
        };
        div()
            .id("shell-panes")
            .debug_selector(if self.view == ShellView::Files { || "files-view".into() } else { || "sessions-view".into() })
            .flex()
            .size_full()
            .min_h_0()
            .on_drag_move::<Edge>(cx.listener(|this, event: &gpui_kit::DragMoveEvent<Edge>, _, cx| {
                // The pointer is in window pixels; the widths are design pixels, which the zoom turns into window pixels.
                let x = atelier_ui::scale::design(event.event.position.x - event.bounds.origin.x) - atelier_ui::view_rail::WIDTH;
                match event.drag(cx) {
                    Edge::Sidebar => this.sidebar_width = x.clamp(super::fit::SIDEBAR_LEAST, super::fit::SIDEBAR_MOST),
                    Edge::Right => this.right_width = (atelier_ui::scale::design(event.bounds.size.width) - x).clamp(super::fit::RIGHT_LEAST, super::fit::RIGHT_MOST),
                }
                cx.notify();
            }))
            .child(self.view_rail(widths.sidebar.is_some(), cx))
            .children(widths.sidebar.map(|w| div().relative().flex_none().w(px(w)).h_full().pl(px(super::types::PANE_GAP)).child(div().size_full().rounded(radius::xl()).overflow_hidden().bg(cx.theme().card).child(self.part("sidebar", self.sidebar(cx).into_any_element()))).child(handle(Edge::Sidebar))))
            .child(div().flex_1().min_w_0().h_full().child(self.part("panels", self.center(project, window, cx))))
            .children(widths.right.map(|w| div().relative().flex_none().w(px(w)).h_full().child(self.part("right", self.right_pane(project, cx))).child(handle(Edge::Right))))
            .into_any_element()
    }

    /// One pane at a time, with a tab for each: the sidebar, the sessions, and the editor or what
    /// stands in its place.
    fn narrow_panes(&mut self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // The right pane has a tab only while it holds something: the pull requests or the tasks. The
        // editor is the Files view's.
        let right = {
            let p = project.read(cx);
            match p.front() {
                crate::open_project::front::Front::Pulls => Some("Pull requests"),
                crate::open_project::front::Front::Tasks => Some("Tasks"),
                crate::open_project::front::Front::Editor => None,
            }
        };
        let panes: Vec<Pane> = [Pane::Projects, Pane::Session].into_iter().chain(right.map(|_| Pane::Right)).collect();
        if right.is_none() && self.narrow == Pane::Right {
            self.narrow = Pane::Session;
        }
        let segment = |label: &'static str, cap: Option<&'static str>| {
            let name = match label {
                "Projects" => "narrow-tab-Projects",
                "Session" => "narrow-tab-Session",
                "Pull requests" => "narrow-tab-Pull requests",
                _ => "narrow-tab-Tasks",
            };
            let segment = Segment::new(label).debug_name(name);
            match cap {
                Some(cap) => segment.cap(keys::cap(cap)),
                None => segment,
            }
        };
        let this = cx.entity();
        let mut segments = vec![segment("Projects", Some("⌘b")), segment("Session", None)];
        segments.extend(right.map(|label| segment(label, Some("⌘⇧b"))));
        let shown = panes.clone();
        let tabs = div().flex().flex_none().items_center().px(px(8.)).h(px(44.)).child(
            Segmented::new("narrow-panes", segments, panes.iter().position(|p| *p == self.narrow).unwrap_or(0)).on_change(move |i, _, cx| {
                this.update(cx, |this, cx| {
                    this.narrow = shown[i];
                    cx.notify();
                })
            }),
        );
        let body = match self.narrow {
            Pane::Projects => self.sidebar(cx).into_any_element(),
            Pane::Session => {
                let total = atelier_ui::scale::design(window.viewport_size().width);
                self.footer_plan = (8., None, None);
                self.vitals.update(cx, |v, cx| {
                    if v.set_columns(None, None) {
                        cx.notify();
                    }
                });
                self.panels.update(cx, |p, cx| {
                    p.set_inset_left(8., cx);
                    p.set_inset_bottom(0., cx);
                    p.fit_to(total - 16., cx)
                });
                self.center(project, window, cx)
            }
            Pane::Right => self.right_pane(project, cx),
        };
        div()
            .debug_selector(|| "sessions-view".into())
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(tabs)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    /// The view's main area: the board, the panels, or the review.
    fn center(&self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.view {
            ShellView::Tasks => self.tasks_main(project, window, cx),
            ShellView::Messages => self.messages_main(),
            ShellView::Mail => self.mail_main(),
            ShellView::Usage => self.usage_main(cx),
            ShellView::Git => self.changes_main(project, cx),
            ShellView::Files => self.files_editor(project, false, cx),
            ShellView::Pulls => self.pulls_main(project, cx),
            ShellView::History => self.history_main(project, cx),
            _ => self.agent_panel(cx),
        }
    }

    /// The right pane for `project`, drawn from its last frame until the project changes.
    fn right_pane(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        self.right_view.update(cx, |pane, cx| pane.show(project, cx));
        crate::view_cache::draw(&self.right_view)
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let built = std::time::Instant::now();
        // A key goes up from the focus, so with nothing focused (a closed form, a pane that went away) no
        // chord would reach the shell: the shell takes the focus back.
        if window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        let root = self.root(window, cx);
        if let Some(meter) = &self.meter {
            crate::frame_meter::add_part(meter, "shell-render", built.elapsed());
        }
        match self.meter.clone() {
            Some(meter) => crate::frame_meter::Timed { child: root, meter }.into_any_element(),
            None => root,
        }
    }
}

impl Shell {
    fn root(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // Rems follow the zoom too, so what is written in them scales with the rest.
        window.set_rem_size(gpui_kit::px(16. * atelier_ui::scale::zoom()));
        self.width = atelier_ui::scale::design(window.viewport_size().width);
        // The side panes keep their width when the other hides; the agent panel takes what is left.
        let body = match self.active().cloned() {
            None => self.start_screen(window, cx).into_any_element(),
            Some(project) => self.panes(&project, window, cx),
        };
        let title_tabs = self.title_tabs(cx);
        self.panels.update(cx, |p, cx| p.set_tabs_hoisted(title_tabs == TitleTabs::Sessions, cx));
        let link_down = self.active().and_then(|p| match &p.read(cx).link {
            atelier_project::Link::Down(why) => Some((p.read(cx).location.place(), why.clone())),
            atelier_project::Link::Up => None,
        });
        let banner = link_down.map(|(place, why)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .mx(px(8.))
                .mb(px(8.))
                .px(px(12.))
                .py(px(8.))
                .rounded(radius::lg())
                .bg(theme.card_strong)
                .text_size(TextSize::Xs.font_size())
                .child(atelier_ui::spinner::Spinner::new("reconnecting").size(px(12.)).color(theme.warning))
                .child(format!("Lost {place} ({why}). Reconnecting; your unsaved edits are kept here."))
        });
        div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::check_for_updates))
            .on_action(cx.listener(Self::open_ssh_form))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::new_session_key))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_right))
            .on_action(cx.listener(Self::pull_requests_key))
            .on_action(cx.listener(|this, _: &ShowSessions, window, cx| this.show_view(ShellView::Sessions, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_to(atelier_ui::scale::zoom() + atelier_ui::scale::STEP, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_to(atelier_ui::scale::zoom() - atelier_ui::scale::STEP, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.zoom_to(1., cx)))
            .on_action(cx.listener(Self::open_tasks_key))
            .on_key_down(cx.listener(Self::key_down))
            .on_modifiers_changed(|event, _, cx| crate::agent_session::dictation::key_modifiers(&event.modifiers, cx))
            .capture_key_down(|_, _, cx| crate::agent_session::dictation::key_other(cx))
            .capture_any_mouse_down(|_, _, cx| crate::agent_session::dictation::key_other(cx))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(FONT_FAMILY)
            .relative()
            .child(self.title_bar(title_tabs, cx))
            .children(banner)
            .child(div().flex().flex_1().min_h_0().child(body))
            .children(self.footer())
            .children((self.settings.is_none() && self.active().is_some()).then(|| self.said.clone()).flatten().map(|words| {
                // A notice floats over the foot of the window: it is not a bar that takes the room.
                div()
                    .absolute()
                    .bottom(px(16.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .debug_selector(|| "notice".into())
                            .max_w(px(560.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(radius::lg())
                            .bg(theme.popover)
                            .shadow(atelier_ui::theme::popover_shadow(&theme))
                            .text_size(TextSize::Sm.font_size())
                            .text_color(theme.foreground)
                            .child(words),
                    )
            }))
            .children(self.tree_menu(cx))
            .children(self.update_panel(window, cx))
            .children(self.opened.as_ref().map(|(_, page)| page.clone()))
            .children(self.settings.as_ref().map(|(pane, _)| div().absolute().top(px(TITLE_BAR)).left_0().right_0().bottom_0().occlude().child(pane.clone())))
            // The dialogs share the Modal: a scrim, Escape and a press on the scrim close it, and focus goes back.
            .children(self.ssh.as_ref().map(|(form, _)| {
                let this = cx.entity().downgrade();
                let focus = form.read(cx).focus_handle(cx);
                Modal::new("open-over-ssh")
                    .view(form.read(cx).view_key())
                    .width(440.)
                    .focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_ssh(window, cx))))
                    .child(form.clone())
            }))
            .children(self.folder.as_ref().map(|(picker, _)| {
                let this = cx.entity().downgrade();
                let focus = picker.read(cx).focus_handle(cx);
                Modal::new("open-folder-picker")
                    .width(520.)
                    .focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_folder_picker(window, cx))))
                    .child(picker.clone())
            }))
            .children(self.icon.as_ref().map(|(picker, _, _)| {
                let this = cx.entity().downgrade();
                let focus = picker.read(cx).focus_handle(cx);
                Modal::new("choose-icon")
                    .width(520.)
                    .focus(&focus)
                    .on_close(move |window, cx| drop(this.update(cx, |shell, cx| shell.close_icon_picker(window, cx))))
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
