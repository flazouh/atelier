//! The Code lens's History view: the checked-out branch's commits in the sidebar, the picked one in full
//! on the main card.

use atelier_ui::scale::px;
use atelier_ui::theme::{ActiveTheme, radius};
use atelier_ui::typography::TextSize;
use atelier_ui::{FileDiff, FileDiffStatus};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Entity, FontWeight, SharedString, div};

use super::structs::Shell;
use crate::history::Read;
use crate::open_project::OpenProject;

/// A note in the middle of a pane: why there is nothing to show yet.
fn note(words: impl Into<SharedString>, muted: gpui_kit::Hsla) -> AnyElement {
    div()
        .flex()
        .size_full()
        .items_center()
        .justify_center()
        .px(px(16.))
        .text_size(TextSize::Xs.font_size())
        .text_color(muted)
        .child(words.into())
        .into_any_element()
}

/// The rows of a file a commit shows before the rest waits for a press.
const SHOWN_ROWS: usize = 200;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Shell {
    /// The History view's sidebar part: one row a commit, its subject over its short sha, author and age.
    pub(super) fn history_list(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // A project the switcher just named, or the view as the window opens, reads its log on the first look.
        if project.read(cx).log.is_none() {
            project.update(cx, |p, cx| p.load_log(cx));
        }
        let p = project.read(cx);
        let commits = match &p.log {
            Some(Read::Ready(commits)) if commits.is_empty() => return note("No commits yet.", theme.muted_foreground),
            Some(Read::Ready(commits)) => commits,
            Some(Read::Failed(why)) => return note(why.clone(), theme.muted_foreground),
            _ => return note("Reading the history…", theme.muted_foreground),
        };
        let picked = p.commit.as_ref().map(|(sha, _)| sha.clone());
        let now = now();
        let rows = commits.iter().map(|c| {
            let on = picked.as_ref() == Some(&c.sha);
            let sha = c.sha.clone();
            let project = project.downgrade();
            div()
                .id(SharedString::from(format!("commit-{}", c.short)))
                .flex()
                .flex_col()
                .gap(px(1.))
                .px(px(8.))
                .py(px(5.))
                .rounded(radius::md())
                .cursor_pointer()
                .when(on, |d| d.bg(theme.card_strong))
                .when(!on, |d| d.hover(|s| s.bg(theme.card_strong.opacity(0.6))))
                .child(div().truncate().text_size(TextSize::Sm.font_size()).child(c.subject.clone()))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .text_size(TextSize::Xs.font_size())
                        .text_color(theme.muted_foreground)
                        .child(div().font_family(atelier_ui::MONO_FONT_FAMILY).child(c.short.clone()))
                        .child(div().flex_1().min_w_0().truncate().child(c.author.clone()))
                        .child(div().flex_none().child(atelier_forge::time::ago(now, c.at))),
                )
                .on_click(move |_, _, cx| drop(project.update(cx, |p, cx| p.show_commit(sha.clone(), cx))))
        });
        div()
            .id("history-list")
            .debug_selector(|| "history-list".into())
            .flex()
            .flex_col()
            .gap(px(1.))
            .size_full()
            .overflow_y_scroll()
            .p(px(6.))
            .children(rows)
            .into_any_element()
    }

    /// The History view's main area: the picked commit's message, then each file it changed.
    pub(super) fn history_main(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let body = match project.read(cx).commit.as_ref() {
            None => note("Pick a commit on the left.", muted),
            Some((_, Read::Reading)) => note("Reading the commit…", muted),
            Some((_, Read::Failed(why))) => note(why.clone(), muted),
            Some((sha, Read::Ready(shown))) => {
                let (subject, rest) = shown.message.split_once('\n').unwrap_or((&shown.message, ""));
                let commit = project.read(cx).log.as_ref().and_then(|log| match log {
                    Read::Ready(commits) => commits.iter().find(|c| c.sha == *sha).cloned(),
                    _ => None,
                });
                let byline = commit.map(|c| format!("{} · {} · {}", c.short, c.author, atelier_forge::time::ago(now(), c.at)));
                let head = div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pb(px(8.))
                    .child(div().text_size(TextSize::Base.font_size()).font_weight(FontWeight::MEDIUM).child(subject.to_string()))
                    .children(byline.map(|b| div().text_size(TextSize::Xs.font_size()).text_color(muted).child(b)))
                    .when(!rest.trim().is_empty(), |d| {
                        d.child(div().pt(px(4.)).text_size(TextSize::Sm.font_size()).text_color(muted).child(rest.trim().to_string()))
                    });
                // Plain rows, all of a file up to a long one: a list that scrolls inside the card would scroll
                // inside the commit, which scrolls already.
                let files = shown.files.iter().enumerate().map(|(i, f)| {
                    FileDiff::new(SharedString::from(format!("commit-file-{i}")), f.path.clone(), f.lines.clone())
                        .preview_rows(f.lines.len().min(SHOWN_ROWS))
                        .status(FileDiffStatus::Complete)
                        .collapse_on_complete(false)
                })
                // In a column of a fixed height, a card would shrink to fit; each keeps its own and the column scrolls.
                .map(|diff| div().flex_none().child(diff));
                div()
                    .id("history-commit")
                    .debug_selector(|| "history-commit".into())
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .size_full()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .child(head)
                    .when(shown.files.is_empty(), |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("No file changed.")))
                    .children(files)
                    .into_any_element()
            }
        };
        self.code_card(body, cx)
    }
}
