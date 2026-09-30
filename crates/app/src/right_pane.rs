//! The right pane as a view of its own, so it is drawn from its last frame until its project changes
//! (`plans/view-cache.md`). It shows the front the project names: the review, the pull requests, the tasks,
//! or the editor. The review, the hub, the Tasks pane and the editors are entities that redraw themselves; the
//! rest (the front, the tabs, a buffer's banners) is the project's, which it observes.

use beui::theme::{ActiveTheme, radius};
use gpui_kit::{Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div, px};

use crate::{
    editor_pane::editor_pane,
    open_project::{OpenProject, front::Front},
};

#[derive(Default)]
pub struct RightPane {
    shown: Option<(Entity<OpenProject>, Subscription)>,
}

impl RightPane {
    /// Shows `project`'s front from now on. It draws again only when the project changes.
    pub fn show(&mut self, project: &Entity<OpenProject>, cx: &mut Context<Self>) {
        if self.shown.as_ref().is_some_and(|(p, _)| p == project) {
            return;
        }
        let watch = cx.observe(project, |_, _, cx| cx.notify());
        self.shown = Some((project.clone(), watch));
        cx.notify();
    }
}

impl Render for RightPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some((project, _)) = &self.shown else { return div().size_full() };
        let p = project.read(cx);
        let pulls = p.pulls.as_ref().filter(|pulls| pulls.shown).map(|pulls| pulls.hub.clone());
        // The review and the pull requests draw their own cards on the page; the last one asked shows.
        let inner = match (p.front(), p.review.as_ref(), pulls) {
            (Front::Review, Some((pane, _)), _) => div().size_full().pt(px(8.)).child(pane.clone()),
            (Front::Pulls, _, Some(hub)) => div().size_full().pt(px(8.)).child(hub),
            (Front::Tasks, _, _) if p.tasks.is_some() => div().size_full().children(p.tasks.as_ref().map(|t| t.pane.clone())),
            _ => div().size_full().pt(px(8.)).rounded(radius::LG).bg(theme.card).child(editor_pane(project, cx)),
        };
        div().size_full().pr(px(8.)).pb(px(4.)).child(inner)
    }
}
