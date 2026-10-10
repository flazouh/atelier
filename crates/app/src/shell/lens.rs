//! What the lenses add to the window: the project switcher in the title bar, the Tasks and Code lenses'
//! sidebars, and the pull requests. Sessions are every project's; Tasks and Code are about the project the switcher names.

use atelier_ui::button::{Button, ButtonSize, ButtonVariant};
use atelier_ui::button_group::ButtonGroup;
use atelier_ui::menu::{self, Choice, Entry, Menu, MenuItem, MenuLook, Origin};
use atelier_ui::popover::{Hang, Popover};
use atelier_ui::project_badge::ProjectBadge;
use atelier_ui::scale::px;
use atelier_ui::session_status::SessionStatus;
use atelier_ui::sidebar_model::Badge;
use atelier_ui::theme::{ActiveTheme, radius};
use atelier_ui::task_marks::{TaskStatusMark, label_tone_color};
use atelier_ui::task_model::TaskStatus;
use atelier_ui::typography::TextSize;
use atelier_ui::{Icon, IconName};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, SharedString, Window, div};

use super::helpers::{ProjectMark, project_mark};
use super::structs::Shell;
use super::view::ShellView;
use crate::agents_view;
use crate::open_project::OpenProject;
use atelier_project::Link;
use atelier_settings::Location;
use crate::tasks::pane::Scope;

/// A row of a lens's sidebar: a mark, the words, and a count at the end. `id` names it for the control socket.
pub(super) fn nav_row(id: String, on: bool, mark: AnyElement, label: SharedString, count: Option<usize>, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    nav_row_marked(id, on, mark, 15., label, count, cx)
}

/// A row whose mark takes `mark_side` pixels: a face, where an icon's 15 would be too small to read.
pub(super) fn nav_row_marked(id: String, on: bool, mark: AnyElement, mark_side: f32, label: SharedString, count: Option<usize>, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let theme = cx.theme();
    let name = id.clone();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || name.clone())
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .rounded(radius::md())
        .cursor_pointer()
        .text_size(TextSize::Sm.font_size())
        .when(on, |d| d.bg(theme.card_strong))
        .when(!on, |d| d.hover(|s| s.bg(theme.card_strong.opacity(0.6))))
        .child(div().flex_none().flex().items_center().justify_center().size(px(mark_side)).text_color(theme.muted_foreground).child(mark))
        .child(div().flex_1().min_w_0().truncate().child(label))
        .children(count.map(|n| div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(n.to_string())))
}

/// A heading over a group of rows in a lens's sidebar.
pub(super) fn nav_heading(words: &'static str, cx: &App) -> AnyElement {
    div()
        .px(px(8.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(TextSize::Xs.font_size())
        .text_color(cx.theme().muted_foreground)
        .child(words)
        .into_any_element()
}

/// Where a project over SSH lives, and whether its link is down; `None` for a local one.
fn remote_place(project: &OpenProject) -> Option<(SharedString, bool)> {
    match &project.location {
        Location::Ssh { host, .. } => Some((host.clone().into(), matches!(project.link, Link::Down(_)))),
        Location::Local { .. } => None,
    }
}

/// The server mark and the host of a project over SSH, as the sidebar shows them; in the warning colour
/// while the link is down.
fn place_mark(host: SharedString, down: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let ink = if down { theme.warning } else { theme.muted_foreground };
    div()
        .debug_selector(|| "switcher-host".into())
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(TextSize::Xs.font_size())
        .text_color(ink)
        .child(Icon::new(IconName::Dns).size(px(12.)).color(ink))
        .child(if down { SharedString::from(format!("{host} · Reconnecting…")) } else { host })
        .into_any_element()
}

/// The switcher's face after a remote project's name.
fn host_mark(project: &OpenProject, cx: &App) -> Option<AnyElement> {
    remote_place(project).map(|(host, down)| place_mark(host, down, cx))
}

/// The end of a switcher row, on its one line: where the project lives when it is remote, then how many of
/// its sessions are open or wait, in the warning colour when they wait.
fn row_end(place: Option<(SharedString, bool)>, count: Option<(String, bool)>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .children(place.map(|(host, down)| place_mark(host, down, cx)))
        .children(count.map(|(words, waits)| {
            div().text_size(TextSize::Xs.font_size()).text_color(if waits { theme.warning } else { theme.muted_foreground }).child(words)
        }))
        .into_any_element()
}

/// The badge of a project with its mark at the corner: a session that needs the reader, or one at work.
fn marked_badge(badge: &Badge, mark: ProjectMark, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let dot = match mark {
        ProjectMark::NeedsYou => Some(theme.warning),
        ProjectMark::Working => Some(theme.accent),
        ProjectMark::Quiet => None,
    };
    div()
        .relative()
        .flex_none()
        .child(ProjectBadge::new(badge.label.clone(), badge.color).icon(badge.icon.clone()))
        .children(dot.map(|dot| {
            div().absolute().right(px(-2.)).bottom(px(-2.)).size(px(7.)).rounded_full().bg(dot).border_1().border_color(theme.background)
        }))
        .into_any_element()
}

impl Shell {
    /// The projects' badges, in the order of `self.projects`.
    pub(super) fn project_badges(&self, cx: &App) -> Vec<Badge> {
        agents_view::badges(&self.projects, &self.badges, cx)
    }

    /// How a project's sessions stand, for the dot on its badge.
    fn mark_of(project: &Entity<OpenProject>, cx: &App) -> ProjectMark {
        let statuses: Vec<SessionStatus> = project.read(cx).sessions.iter().map(|s| s.read(cx).status.clone()).collect();
        project_mark(&statuses)
    }

    /// The sessions of every project that wait on the reader, for the count on the Sessions lens.
    pub(super) fn needs_you(&self, cx: &App) -> usize {
        self.projects.iter().flat_map(|p| p.read(cx).sessions.iter()).filter(|s| s.read(cx).status.needs_you()).count()
    }

    /// The project the switcher names: in Sessions the one the list is narrowed to (none for all of them),
    /// elsewhere the project the view is about.
    pub(super) fn switched(&self, cx: &App) -> Option<usize> {
        if self.view == ShellView::Sessions {
            let place = self.session_filter.as_ref()?;
            return self.projects.iter().position(|p| agents_view::project_id(p.read(cx)) == *place);
        }
        self.active().map(|_| self.active)
    }

    /// The project switcher at the left of the title bar. `None` before a project is open.
    pub(super) fn project_switcher(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.projects.is_empty() || self.settings.is_some() {
            return None;
        }
        let badges = self.project_badges(cx);
        let current = self.switched(cx);
        let sessions = self.view == ShellView::Sessions;
        let face = match current {
            Some(at) => div()
                .flex()
                .items_center()
                .min_w_0()
                .gap(px(7.))
                .child(marked_badge(&badges[at], Self::mark_of(&self.projects[at], cx), cx))
                .child(div().min_w_0().truncate().child(self.projects[at].read(cx).name()))
                .children(host_mark(self.projects[at].read(cx), cx).map(|mark| div().flex_none().child(mark))),
            None => div().child("All projects"),
        };
        let this = cx.entity().downgrade();
        let menu = self.switcher_open.then(|| {
            let mut entries: Vec<Entry> = Vec::new();
            if sessions {
                let all = this.clone();
                let count = self.projects.iter().map(|p| p.read(cx).sessions.len()).sum::<usize>();
                entries.push(Entry::from(
                    MenuItem::new("All projects")
                        .debug_name("switcher-all")
                        .trailing(move |cx| row_end(None, Some((format!("{count} open"), false)), cx))
                        .choice(Choice::Radio(current.is_none()))
                        .on_select(move |window, cx| drop(all.update(cx, |s, cx| s.switch_project(None, window, cx)))),
                ));
                entries.push(Entry::Separator);
            }
            entries.push(Entry::Label("Projects".into()));
            for (at, project) in self.projects.iter().enumerate() {
                let (badge, mark) = (badges[at].clone(), Self::mark_of(project, cx));
                let pick = this.clone();
                let p = project.read(cx);
                let waiting = p.sessions.iter().filter(|s| s.read(cx).status.needs_you()).count();
                let item = MenuItem::new(p.name())
                    .debug_name(format!("switcher-{}", p.name()))
                    .lead_element(move |cx| marked_badge(&badge, mark, cx))
                    .choice(Choice::Radio(current == Some(at)))
                    .on_select(move |window, cx| drop(pick.update(cx, |s, cx| s.switch_project(Some(at), window, cx))));
                let count = match (waiting, p.sessions.len()) {
                    (0, 0) => None,
                    (0, open) => Some((format!("{open} open"), false)),
                    (waiting, _) => Some((format!("{waiting} need you"), true)),
                };
                let place = remote_place(p);
                let item = if place.is_none() && count.is_none() { item } else { item.trailing(move |cx| row_end(place.clone(), count.clone(), cx)) };
                entries.push(Entry::from(item));
            }
            let close = this.clone();
            Popover::new("project-switcher-popover")
                .open(true)
                .hang(Hang::Left(0., 30.))
                .keep_focus()
                .height(menu::height_of(MenuLook::SELECT, &entries))
                .on_close(move |_, cx| {
                    drop(close.update(cx, |s, cx| {
                        s.switcher_open = false;
                        cx.notify();
                    }))
                })
                .child(Menu::new("project-switcher-menu", entries).look(MenuLook::SELECT).min_width(240.).origin(Origin::TopLeft))
        });
        let toggle = this.clone();
        let button = Button::new("project-switcher")
            .debug_name("project-switcher")
            .content(face)
            .shrink(true)
            .trailing_icon(IconName::ChevronDown)
            .open(self.switcher_open)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                drop(toggle.update(cx, |s, cx| {
                    s.switcher_open = !s.switcher_open;
                    s.add_open = false;
                    cx.notify();
                }))
            });
        let toggle = this.clone();
        let add = Button::new("add-project")
            .debug_name("add-project")
            .icon(IconName::Add)
            .tooltip("Add a project")
            .open(self.add_open)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                drop(toggle.update(cx, |s, cx| {
                    s.add_open = !s.add_open;
                    s.switcher_open = false;
                    cx.notify();
                }))
            });
        let group = ButtonGroup::new("project-switcher-group").fit(true).variant(ButtonVariant::Secondary).size(ButtonSize::Sm).child(button).child(add);
        Some(
            div()
                .relative()
                .min_w_0()
                .child(crate::control::marked("project-switcher", group))
                .children(menu)
                .children(self.add_open.then(|| self.add_menu(cx)))
                .into_any_element(),
        )
    }

    /// What the add button next to the switcher offers: a folder on this machine, or one over SSH.
    fn add_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let ask = |label: &'static str, debug: &'static str, cap: &'static str, remote: bool| {
            let shell = this.clone();
            Entry::from(MenuItem::new(label).debug_name(debug).cap(atelier_ui::keys::cap(cap)).on_select(move |window, cx| {
                drop(shell.update(cx, |s, cx| {
                    s.add_open = false;
                    if remote { s.open_ssh_form(&super::structs::OpenRemote, window, cx) } else { s.open_folder(&super::structs::OpenFolder, window, cx) }
                    cx.notify();
                }))
            }))
        };
        let entries = vec![ask("Open folder…", "add-folder", "⌘o", false), ask("Open over SSH…", "add-ssh", "⌘⇧o", true)];
        let close = this.clone();
        Popover::new("add-project-popover")
            .open(true)
            .hang(Hang::Left(0., 30.))
            .keep_focus()
            .height(menu::height_of(MenuLook::SELECT, &entries))
            .on_close(move |_, cx| {
                drop(close.update(cx, |s, cx| {
                    s.add_open = false;
                    cx.notify();
                }))
            })
            .child(Menu::new("add-project-menu", entries).look(MenuLook::SELECT).min_width(200.).origin(Origin::TopLeft))
            .into_any_element()
    }

    /// The switcher's choice: in Sessions it narrows the list and the panels to one project, or shows them all;
    /// elsewhere it is the project the view is about.
    pub(super) fn switch_project(&mut self, at: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.switcher_open = false;
        if self.view == ShellView::Sessions {
            self.session_filter = at.and_then(|at| self.projects.get(at)).map(|p| agents_view::project_id(p.read(cx)));
            if let Some(at) = at {
                self.active = at;
            }
            self.sync(cx);
            return;
        }
        if let Some(at) = at.filter(|at| *at < self.projects.len()) {
            self.active = at;
            if self.view == ShellView::Pulls {
                self.projects[at].update(cx, |p, cx| p.load_pulls(window, cx));
            }
        }
        cx.notify();
    }

    /// The Code lens's sidebar: its three views, then what the one in front lists.
    pub(super) fn code_sidebar(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let p = project.read(cx);
        let pulls = p.pulls.as_ref().map(|_| p.list_rows.len());
        let in_session = self.focused(cx).map_or(0, |(_, s)| s.read(cx).changed_files().len());
        let changes = super::helpers::changes_badge(p.uncommitted.as_ref(), in_session);
        let row = |view: ShellView, icon: IconName, label: &'static str, count: Option<usize>, cx: &mut Context<Self>| {
            let this = cx.entity().downgrade();
            nav_row(format!("code-nav-{}", view.words()), self.view == view, Icon::new(icon).into_any_element(), label.into(), count, cx)
                .on_click(move |_, window, cx| drop(this.update(cx, |s, cx| s.show_code(view, window, cx))))
        };
        let nav = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .p(px(6.))
            .child(row(ShellView::Pulls, IconName::PrOpen, "Pull requests", pulls, cx))
            .child(row(ShellView::Files, IconName::Folder, "Files", None, cx))
            .child(row(ShellView::History, IconName::Schedule, "History", None, cx))
            .child(row(ShellView::Git, IconName::Commit, "Changes", changes, cx));
        let below = match self.view {
            ShellView::Files => Some(self.files_tree(project, cx)),
            ShellView::History => Some(self.history_list(project, cx)),
            ShellView::Git => Some(self.changes_list(project, cx)),
            _ => None,
        };
        div()
            .debug_selector(|| "code-sidebar".into())
            .flex()
            .flex_col()
            .size_full()
            .child(nav)
            .children(below.map(|below| div().flex_1().min_h_0().border_t_1().border_color(theme.background).child(below)))
            .into_any_element()
    }

    /// The Tasks lens's sidebar: the views over the project's tasks, its labels, and the agents that hold
    /// issues. Each sets the scope of the project's Tasks pane.
    pub(super) fn issues_sidebar(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let Some(pane) = project.read(cx).tasks.as_ref().map(|slot| slot.pane.clone()) else {
            return div().into_any_element();
        };
        let theme = cx.theme().clone();
        let p = pane.read(cx);
        let scope = p.scope().clone();
        let row = |id: String, at: Scope, mark: AnyElement, label: SharedString, cx: &App| {
            let pane = pane.downgrade();
            let count = Some(p.count(&at)).filter(|n| *n > 0);
            nav_row(id, scope == at, mark, label, count, cx)
                .on_click(move |_, window, cx| drop(pane.update(cx, |p, cx| p.set_scope(at.clone(), window, cx))))
                .into_any_element()
        };
        let views = Scope::VIEWS.map(|at| {
            let (id, label, mark) = match at {
                Scope::Mine => ("mine", "My tasks", Icon::new(IconName::VerifiedUser).into_any_element()),
                Scope::Active => ("active", "Active", TaskStatusMark::new(TaskStatus::InProgress).size(px(14.)).into_any_element()),
                Scope::Backlog => ("backlog", "Backlog", TaskStatusMark::new(TaskStatus::Backlog).size(px(14.)).into_any_element()),
                _ => ("all", "All tasks", Icon::new(IconName::FormatListBulleted).into_any_element()),
            };
            row(format!("issues-{id}"), at, mark, label.into(), cx)
        });
        let labels: Vec<AnyElement> = p
            .labels()
            .iter()
            .map(|l| {
                let dot = div().size(px(8.)).rounded_full().bg(label_tone_color(l, &theme)).into_any_element();
                row(format!("issues-label-{}", l.name), Scope::Label(l.name.clone()), dot, l.name.clone(), cx)
            })
            .collect();
        let agents: Vec<AnyElement> = p
            .agents_at_work()
            .into_iter()
            .map(|(name, _)| row(format!("issues-agent-{name}"), Scope::Agent(name.clone()), Icon::new(IconName::Bot).into_any_element(), name, cx))
            .collect();
        div()
            .debug_selector(|| "issues-sidebar".into())
            .id("issues-sidebar")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .gap(px(1.))
            .p(px(6.))
            .children(views)
            .when(!labels.is_empty(), |d| d.child(nav_heading("Labels", cx)).children(labels))
            .when(!agents.is_empty(), |d| d.child(nav_heading("Agents", cx)).children(agents))
            .into_any_element()
    }

    /// The lens in front.
    pub(crate) fn view(&self) -> ShellView {
        self.view
    }

    /// Brings `view` to the front as the rail or the Code sidebar would.
    pub(crate) fn go_to(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        match view {
            ShellView::Tasks => self.show_tasks(window, cx),
            ShellView::Usage => self.show_usage(window, cx),
            ShellView::Bots => self.show_bots(window, cx),
            v if v.in_code() => self.show_code(v, window, cx),
            v => self.show_view(v, window, cx),
        }
    }

    /// Brings a view of the Code lens to the front, and remembers it for the next press on the rail.
    pub(super) fn show_code(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        self.code_view = view;
        match view {
            ShellView::Git => {
                self.show_git(window, cx);
                if let Some(project) = self.active().cloned() {
                    project.update(cx, |p, cx| p.load_uncommitted(cx));
                }
            }
            ShellView::Pulls => {
                self.show_view(view, window, cx);
                if let Some(project) = self.active().cloned() {
                    project.update(cx, |p, cx| p.load_pulls(window, cx));
                }
            }
            ShellView::History => {
                self.show_view(view, window, cx);
                if let Some(project) = self.active().cloned() {
                    project.update(cx, |p, cx| p.load_log(cx));
                }
            }
            _ => self.show_view(view, window, cx),
        }
    }

    /// The Code lens's pull requests: the project's list and the one open, or why there is none.
    pub(super) fn pulls_main(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let p = project.read(cx);
        if let Some(pulls) = &p.pulls {
            return div().debug_selector(|| "code-pulls".into()).size_full().pl(px(super::types::PANE_GAP)).pr(px(8.)).child(pulls.hub.clone()).into_any_element();
        }
        let muted = cx.theme().muted_foreground;
        let words: SharedString = match p.pulls_unavailable() {
            Some(why) => why.into(),
            None if p.pulls_loading() => "Reading pull requests…".into(),
            None => "Pull requests open here.".into(),
        };
        self.code_card(div().flex().size_full().items_center().justify_center().text_size(TextSize::Xs.font_size()).text_color(muted).child(words).into_any_element(), cx)
    }

    /// A main area of the Code lens on its card, 2 px from the sidebar's card like the panels.
    pub(super) fn code_card(&self, child: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let card = cx.theme().card;
        div()
            .size_full()
            .pl(px(super::types::PANE_GAP))
            .pr(px(8.))
            
            .child(div().size_full().rounded(radius::xl()).overflow_hidden().bg(card).child(child))
            .into_any_element()
    }
}
