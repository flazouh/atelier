//! The left rail and what its views draw: Tasks shows the board, Sessions the session list and the
//! panels, Git the focused session's changed files and their review. After the app's own three, the rail has one
//! entry for each view a plugin registered.

use std::collections::BTreeMap;
use std::sync::Mutex;

use atelier_plugin::PluginView;
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
use crate::slots::Slots;

/// The entry of one of the app's own views.
fn rail_view(view: ShellView, needs_you: usize) -> RailView {
    match view {
        ShellView::Tasks => RailView { icon: IconName::Checklist, label: "Tasks".into(), debug: "rail-tasks", count: 0 },
        ShellView::Git => RailView { icon: IconName::Code, label: "Code".into(), debug: "rail-git", count: 0 },
        _ => RailView { icon: IconName::Forum, label: "Sessions".into(), debug: "rail-sessions", count: needs_you },
    }
}

/// The debug name of the entry of a plugin's view: `rail-<id>`. The rail takes a name that lasts as long as the app,
/// so the name of each id is made once and kept.
fn plugin_rail_name(id: &'static str) -> &'static str {
    static NAMES: Mutex<BTreeMap<&'static str, &'static str>> = Mutex::new(BTreeMap::new());
    let mut names = NAMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    names.entry(id).or_insert_with(|| Box::leak(format!("rail-{id}").into_boxed_str()))
}

/// The entry of a view a plugin registered.
fn plugin_rail_view(view: &PluginView) -> RailView {
    RailView { icon: view.icon, label: view.label.clone(), debug: plugin_rail_name(view.id), count: 0 }
}

impl Shell {
    /// What the rail lists, top to bottom: the app's own three, then each view a plugin registered, by its order.
    fn rail_entries(&self, cx: &App) -> Vec<(ShellView, RailView)> {
        let needs_you = self.needs_you(cx);
        let own = ShellView::ON_RAIL.into_iter().map(|view| (view, rail_view(view, needs_you)));
        let registered = cx.global::<Slots>().views().into_iter().map(|view| (ShellView::Plugin(view.id), plugin_rail_view(view)));
        own.chain(registered).collect()
    }

    /// The rail, with the view in front marked, folded while the sidebar is hidden.
    pub(super) fn view_rail(&self, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let (on_rail, views): (Vec<ShellView>, Vec<RailView>) = self.rail_entries(cx).into_iter().unzip();
        let selected = on_rail.iter().position(|v| *v == self.view.lens()).unwrap_or(usize::MAX);
        let this = cx.entity().downgrade();
        ViewRail::new("view-rail", views, selected, open)
            .on_select(move |i, window, cx| {
                let Some(view) = on_rail.get(i).copied() else { return };
                this.update(cx, |this, cx| this.pick_view(view, window, cx)).ok();
            })
            .into_any_element()
    }

    /// A press on the rail. Another view comes to the front with the sidebar shown; the view in front
    /// hides or shows the sidebar.
    pub(super) fn pick_view(&mut self, view: ShellView, window: &mut Window, cx: &mut Context<Self>) {
        let fit = Fit::of(atelier_ui::scale::design(window.viewport_size().width));
        if view == self.view.lens() {
            return self.flip_sidebar(fit, cx);
        }
        if !self.sidebar_shown(fit) {
            self.flip_sidebar(fit, cx);
        }
        // The Code lens comes back on the view it was left on.
        let view = if view == ShellView::Git { self.code_view } else { view };
        self.go_to(view, window, cx);
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
