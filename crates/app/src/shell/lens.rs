//! What the lenses add to the window: the project switcher in the title bar, the Code lens's sidebar and its
//! pull requests. Sessions are every project's; Issues and Code are about the project the switcher names.

use atelier_ui::button::{Button, ButtonVariant};
use atelier_ui::menu::{self, Choice, Entry, Menu, MenuItem, MenuLook, Origin};
use atelier_ui::popover::{Hang, Popover};
use atelier_ui::project_badge::ProjectBadge;
use atelier_ui::scale::px;
use atelier_ui::session_status::SessionStatus;
use atelier_ui::sidebar_model::Badge;
use atelier_ui::theme::{ActiveTheme, radius};
use atelier_ui::typography::TextSize;
use atelier_ui::{Icon, IconName};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, SharedString, Window, div};

use super::helpers::{ProjectMark, project_mark};
use super::structs::Shell;
use super::view::ShellView;
use crate::agents_view;
use crate::open_project::OpenProject;

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
    fn project_badges(&self, cx: &App) -> Vec<Badge> {
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
        let theme = cx.theme().clone();
        let badges = self.project_badges(cx);
        let current = self.switched(cx);
        let sessions = self.view == ShellView::Sessions;
        let face = match current {
            Some(at) => div()
                .flex()
                .items_center()
                .gap(px(7.))
                .child(marked_badge(&badges[at], Self::mark_of(&self.projects[at], cx), cx))
                .child(self.projects[at].read(cx).name()),
            None => div()
                .flex()
                .items_center()
                .gap(px(7.))
                .child(div().flex_none().size(px(16.)).rounded(radius::md()).bg(theme.muted_foreground.opacity(0.35)))
                .child("All projects"),
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
                        .description(format!("{count} open sessions"))
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
                    .lead(move |cx| marked_badge(&badge, mark, cx))
                    .choice(Choice::Radio(current == Some(at)))
                    .on_select(move |window, cx| drop(pick.update(cx, |s, cx| s.switch_project(Some(at), window, cx))));
                let item = match (waiting, p.sessions.len()) {
                    (0, 0) => item,
                    (0, open) => item.description(format!("{open} open")),
                    (waiting, _) => item.description(format!("{waiting} need you")),
                };
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
            .trailing_icon(IconName::ChevronDown)
            .variant(ButtonVariant::Ghost)
            .open(self.switcher_open)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                drop(toggle.update(cx, |s, cx| {
                    s.switcher_open = !s.switcher_open;
                    cx.notify();
                }))
            });
        Some(div().relative().child(crate::control::marked("project-switcher", button)).children(menu).into_any_element())
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
        let changes = self.focused(cx).map(|(_, s)| s.read(cx).changed_files().len()).filter(|n| *n > 0);
        let row = |view: ShellView, icon: IconName, label: &'static str, count: Option<usize>, cx: &mut Context<Self>| {
            let on = self.view == view;
            let this = cx.entity().downgrade();
            div()
                .id(SharedString::from(format!("code-nav-{}", view.words())))
                .debug_selector(move || format!("code-nav-{}", view.words()))
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
                .child(Icon::new(icon).size(px(15.)).color(theme.muted_foreground))
                .child(div().flex_1().child(label))
                .children(count.map(|n| div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(n.to_string())))
                .on_click(move |_, window, cx| drop(this.update(cx, |s, cx| s.show_code(view, window, cx))))
        };
        let nav = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .p(px(6.))
            .child(row(ShellView::Pulls, IconName::PrOpen, "Pull requests", pulls, cx))
            .child(row(ShellView::Files, IconName::Folder, "Files", None, cx))
            .child(row(ShellView::Git, IconName::Commit, "Changes", changes, cx));
        let below = match self.view {
            ShellView::Files => Some(self.files_tree(project, cx)),
            ShellView::Git => Some(self.git_sidebar(cx)),
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

    /// Brings `view` to the front as the rail or the Code sidebar would.
    pub(crate) fn go_to(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        match view {
            ShellView::Tasks => self.show_tasks(window, cx),
            v if v.in_code() => self.show_code(v, window, cx),
            v => self.show_view(v, window, cx),
        }
    }

    /// Brings a view of the Code lens to the front, and remembers it for the next press on the rail.
    pub(super) fn show_code(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        self.code_view = view;
        match view {
            ShellView::Git => self.show_git(window, cx),
            ShellView::Pulls => {
                self.show_view(view, window, cx);
                if let Some(project) = self.active().cloned() {
                    project.update(cx, |p, cx| p.load_pulls(window, cx));
                }
            }
            _ => self.show_view(view, window, cx),
        }
    }

    /// The Code lens's pull requests: the project's list and the one open, or why there is none.
    pub(super) fn pulls_main(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let p = project.read(cx);
        if let Some(pulls) = &p.pulls {
            return div().debug_selector(|| "code-pulls".into()).size_full().pl(px(atelier_ui::panel_layout::GAP)).pr(px(8.)).pb(px(8.)).child(pulls.hub.clone()).into_any_element();
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
            .pl(px(atelier_ui::panel_layout::GAP))
            .pr(px(8.))
            .pb(px(8.))
            .child(div().size_full().rounded(radius::xl()).overflow_hidden().bg(card).child(child))
            .into_any_element()
    }
}
