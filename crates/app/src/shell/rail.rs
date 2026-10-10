//! The left rail and what its views draw: Tasks shows the board, Sessions the session list and the
//! panels, Git the focused session's changed files and their review, Messages the chat accounts, Mail the mail accounts.

use atelier_ui::view_rail::{RailView, ViewRail};
use atelier_ui::scale::px;
use atelier_ui::IconName;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, Window, div};

use super::view::ShellView;
use super::fit::Fit;
use super::structs::Shell;
use crate::agent_session::AgentSession;
use crate::open_project::OpenProject;
use crate::review_pane::Scope;

fn rail_view(view: ShellView, needs_you: usize) -> RailView {
    match view {
        ShellView::Tasks => RailView { icon: IconName::Checklist, label: "Tasks".into(), debug: "rail-tasks", count: 0 },
        ShellView::Git => RailView { icon: IconName::Code, label: "Code".into(), debug: "rail-git", count: 0 },
        ShellView::Messages => RailView { icon: IconName::ChatBubble, label: "Messages".into(), debug: "rail-messages", count: 0 },
        ShellView::Mail => RailView { icon: IconName::Mail, label: "Mail".into(), debug: "rail-mail", count: 0 },
        ShellView::Usage => RailView { icon: IconName::BarChart, label: "Usage".into(), debug: "rail-usage", count: 0 },
        _ => RailView { icon: IconName::Forum, label: "Sessions".into(), debug: "rail-sessions", count: needs_you },
    }
}

impl Shell {
    /// The rail, with the view in front marked, folded while the sidebar is hidden.
    pub(super) fn view_rail(&self, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let needs_you = self.needs_you(cx);
        let views = ShellView::ON_RAIL.into_iter().map(|v| rail_view(v, needs_you)).collect();
        let selected = ShellView::ON_RAIL.iter().position(|v| *v == self.view.lens()).unwrap_or(usize::MAX);
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
        // A view a module opened over the window gives way to the lens the reader picks.
        if self.opened.take().is_some() {
            cx.notify();
        }
        let fit = Fit::of(atelier_ui::scale::design(window.viewport_size().width));
        if view == self.view.lens() {
            return self.flip_sidebar(fit, cx);
        }
        if !self.sidebar_shown(fit) {
            self.flip_sidebar(fit, cx);
        }
        match view {
            ShellView::Tasks => self.show_tasks(window, cx),
            ShellView::Git => self.show_code(self.code_view, window, cx),
            ShellView::Messages => self.show_messages(window, cx),
            ShellView::Mail => self.show_mail(window, cx),
            ShellView::Usage => self.show_usage(window, cx),
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

    /// The Tasks view's main area: the board, loaded on the first look.
    pub(super) fn tasks_main(&self, project: &Entity<OpenProject>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if project.read(cx).tasks.is_none() {
            project.update(cx, |p, cx| p.mount_tasks(window, cx));
        }
        match project.read(cx).tasks.as_ref() {
            Some(slot) => div().debug_selector(|| "tasks-view".into()).size_full().pl(px(super::types::PANE_GAP)).pr(px(8.)).child(slot.pane.clone()).into_any_element(),
            None => div().into_any_element(),
        }
    }
}
