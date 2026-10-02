//! The left rail and what its views draw: Tasks shows the board, Sessions the session list and the
//! panels, Git the focused session's changed files and their review.

use atelier_ui::git_panel::GitPanel;
use atelier_ui::view_rail::{RailView, ViewRail};
use atelier_ui::scale::px;
use atelier_ui::theme::ActiveTheme;
use atelier_ui::typography::TextSize;
use atelier_ui::IconName;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, SharedString, Window, div};

use super::view::ShellView;
use super::fit::Fit;
use super::structs::Shell;
use crate::agent_session::AgentSession;
use crate::open_project::OpenProject;
use crate::review_pane::Scope;

fn rail_view(view: ShellView) -> RailView {
    match view {
        ShellView::Tasks => RailView { icon: IconName::Checklist, label: "Tasks".into(), debug: "rail-tasks" },
        ShellView::Git => RailView { icon: IconName::PrOpen, label: "Git".into(), debug: "rail-git" },
        _ => RailView { icon: IconName::Forum, label: "Sessions".into(), debug: "rail-sessions" },
    }
}

impl Shell {
    /// The rail, with the view in front marked, folded while the sidebar is hidden.
    pub(super) fn view_rail(&self, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let views = ShellView::ON_RAIL.into_iter().map(rail_view).collect();
        let selected = ShellView::ON_RAIL.iter().position(|v| *v == self.view).unwrap_or(usize::MAX);
        let this = cx.entity().downgrade();
        ViewRail::new("view-rail", views, selected, open)
            .on_select(move |i, window, cx| {
                this.update(cx, |this, cx| this.pick_view(ShellView::ON_RAIL[i], window, cx)).ok();
            })
            .into_any_element()
    }

    /// A press on the rail. Another view comes to the front with the sidebar shown; the view in front
    /// hides or shows the sidebar.
    pub(super) fn pick_view(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        let fit = Fit::of(atelier_ui::scale::design(window.viewport_size().width));
        if view == self.view {
            return self.flip_sidebar(fit, cx);
        }
        if !self.sidebar_shown(fit) {
            self.flip_sidebar(fit, cx);
        }
        match view {
            ShellView::Tasks => self.show_tasks(window, cx),
            ShellView::Git => self.show_git(window, cx),
            _ => self.show_view(view, window, cx),
        }
    }

    /// The focused panel's session, and its project.
    pub(super) fn focused(&self, cx: &App) -> Option<(Entity<OpenProject>, Entity<AgentSession>)> {
        let key = self.panels.read(cx).active()?;
        let (at, session) = self.session_by_key(key, cx)?;
        Some((self.projects.get(at)?.clone(), session))
    }

    /// The Git view, with the focused session's whole review open when it changed a file and no
    /// review of it is open yet.
    pub(super) fn show_git(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_view(ShellView::Git, window, cx);
        let Some((project, session)) = self.focused(cx) else { return };
        let open = project.read(cx).review.as_ref().is_some_and(|(pane, _)| pane.read(cx).session == session);
        if !open && !session.read(cx).changed_files().is_empty() {
            self.review(&project, session, Scope::Whole, None, window, cx);
        } else if let Some(i) = self.projects.iter().position(|p| *p == project) {
            self.active = i;
        }
    }

    /// Opens the review of `session` in the Git view, at `path` when one is given.
    pub(super) fn review(&mut self, project: &Entity<OpenProject>, session: Entity<AgentSession>, scope: Scope, path: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.projects.iter().position(|p| p == project) {
            self.active = i;
        }
        if self.view != ShellView::Git {
            self.view = ShellView::Git;
            self.save_view(cx);
        }
        project.update(cx, |p, cx| p.open_review(session, scope, path, window, cx));
        self.narrow = super::fit::Pane::Session;
        cx.notify();
    }

    /// A press on a file of the Git sidebar: the open review of that session goes to it, else a
    /// review opens on it.
    fn open_change(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, session)) = self.focused(cx) else { return };
        let pane = project.read(cx).review.as_ref().map(|(pane, _)| pane.clone()).filter(|pane| pane.read(cx).session == session);
        match pane {
            Some(pane) => pane.update(cx, |pane, cx| pane.open(path, window, cx)),
            None => self.review(&project, session, Scope::Whole, Some(path), window, cx),
        }
    }

    /// The Git view's sidebar: the focused session's repository, branch and changed files.
    pub(super) fn git_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((project, session)) = self.focused(cx) else {
            let Some(project) = self.active() else { return GitPanel::new("git-panel", "").into_any_element() };
            let p = project.read(cx);
            return GitPanel::new("git-panel", p.name()).branch(p.git.branch().cloned()).into_any_element();
        };
        let p = project.read(cx);
        let s = session.read(cx);
        let current = p.review.as_ref().map(|(pane, _)| pane.read(cx)).filter(|pane| pane.session == session).and_then(|pane| pane.files.get(pane.current)).map(|f| SharedString::from(f.review.path.clone()));
        let this = cx.entity().downgrade();
        GitPanel::new("git-panel", p.name())
            .branch(p.git.branch().cloned())
            .session(Some(s.title.clone()))
            .files(s.changed_files().to_vec())
            .current(current)
            .on_open(move |path, window, cx| {
                this.update(cx, |this, cx| this.open_change(path, window, cx)).ok();
            })
            .into_any_element()
    }

    /// The Git view's main area: the review, or a word on how to open one.
    pub(super) fn git_main(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        if let Some((pane, _)) = project.read(cx).review.as_ref() {
            return div().debug_selector(|| "review-in-place".into()).size_full().pt(px(8.)).pr(px(8.)).pb(px(4.)).child(pane.clone()).into_any_element();
        }
        let muted = cx.theme().muted_foreground;
        div()
            .debug_selector(|| "git-empty".into())
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .text_size(TextSize::Xs.font_size())
            .text_color(muted)
            .child("Press a changed file to review it.")
            .into_any_element()
    }

    /// The Tasks view's main area: the board, loaded on the first look.
    pub(super) fn tasks_main(&self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if project.read(cx).tasks.is_none() {
            project.update(cx, |p, cx| p.mount_tasks(window, cx));
        }
        match project.read(cx).tasks.as_ref() {
            Some(slot) => div().debug_selector(|| "tasks-view".into()).size_full().pt(px(8.)).pr(px(8.)).pb(px(4.)).child(slot.pane.clone()).into_any_element(),
            None => div().into_any_element(),
        }
    }
}
