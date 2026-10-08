//! The Code lens's History view: the checked-out branch's commits in the sidebar, the picked one in full
//! on the main card.

use atelier_ui::scale::px;
use atelier_ui::theme::{ActiveTheme, radius};
use atelier_ui::typography::TextSize;
use atelier_ui::file_diff::diff_stats;
use atelier_ui::file_tree::FileTree;
use atelier_ui::{ChangedFile, ChangedFileTree, FileDiff, FileDiffStatus};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Entity, FontWeight, SharedString, WeakEntity, canvas, div};

use super::structs::Shell;
use super::view::ShellView;
use crate::history::{CommitFile, Read};
use crate::open_project::OpenProject;

/// A note in the middle of a pane: why there is nothing to show yet.
pub(super) fn note(words: impl Into<SharedString>, muted: gpui_kit::Hsla) -> AnyElement {
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

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// How wide a tree of files sits beside the diff, as in a pull request and a review.
const TREE_WIDTH: f32 = 220.;

/// The row a file diff's own head takes over its rows.
const DIFF_HEAD: f32 = 48.;

/// What a tree lists of `files`: each one's path, counts and kind.
pub(super) fn changed(files: &[CommitFile]) -> Vec<ChangedFile> {
    files
        .iter()
        .map(|f| {
            let (added, removed) = diff_stats(&f.lines);
            ChangedFile::new(f.path.clone(), added, removed).change(f.change.clone())
        })
        .collect()
}

/// The file of `files` to show: the one picked while it is there, else the first the tree lists.
pub(super) fn shown_file<'a>(files: &'a [CommitFile], picked: Option<&SharedString>) -> Option<&'a CommitFile> {
    let find = |path: &SharedString| files.iter().find(|f| f.path == *path);
    picked.and_then(find).or_else(|| FileTree::new(&changed(files)).file_order().first().and_then(find))
}

impl Shell {
    /// Shows `path` in `view`'s diff, as a press on its row in the tree does.
    pub(crate) fn pick_file(&mut self, view: ShellView, path: SharedString, cx: &mut Context<Self>) {
        match view {
            ShellView::History => self.history_file = Some(path),
            _ => {
                // A session's review gives the card back to the checkout's changes first.
                if let Some(project) = self.active().cloned() {
                    project.update(cx, |p, cx| p.close_review(cx));
                }
                self.change_file = Some(path);
            }
        }
        cx.notify();
    }

    /// The tree of `files` for `view`, with `current` washed; a press on a file shows it. It grows with its rows, for
    /// a column that scrolls, or with `fill` it fills the height it is given and draws only the rows in view.
    pub(super) fn file_tree(this: &WeakEntity<Self>, id: &'static str, view: ShellView, files: &[CommitFile], current: &SharedString, fill: bool) -> AnyElement {
        let this = this.clone();
        let tree = ChangedFileTree::new(id, changed(files)).current(current.clone()).on_open(move |path, _, cx| {
            _ = this.update(cx, |s, cx| s.pick_file(view, path.clone(), cx));
        });
        div()
            .debug_selector(move || id.into())
            .when(fill, |d| d.size_full())
            .child(if fill { tree.virtualised() } else { tree })
            .into_any_element()
    }

    /// One file's diff, filling the space left to it: its rows scroll inside it, so a long file costs only the rows
    /// in sight.
    pub(super) fn one_diff(&self, this: &WeakEntity<Self>, file: &CommitFile) -> AnyElement {
        let this = this.clone();
        let measure = canvas(
            move |bounds, _, cx| {
                let height = f32::from(bounds.size.height);
                _ = this.update(cx, |s, cx| {
                    if (s.diff_height - height).abs() > 0.5 {
                        s.diff_height = height;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let selector = format!("file-diff-{}", file.path);
        let diff = FileDiff::new(SharedString::from(selector.clone()), file.path.clone(), file.lines.clone())
            .status(FileDiffStatus::Complete)
            .collapse_on_complete(false)
            .max_height((self.diff_height - DIFF_HEAD).max(120.));
        div().debug_selector(move || selector.clone()).relative().flex_1().min_w_0().min_h_0().child(measure).child(diff).into_any_element()
    }

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
        let this = cx.entity().downgrade();
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
                    
                    .child(div().text_size(TextSize::Base.font_size()).font_weight(FontWeight::MEDIUM).child(subject.to_string()))
                    .children(byline.map(|b| div().text_size(TextSize::Xs.font_size()).text_color(muted).child(b)))
                    .when(!rest.trim().is_empty(), |d| {
                        d.child(
                            div()
                                .id("commit-body")
                                .max_h(px(120.))
                                .overflow_y_scroll()
                                .pt(px(4.))
                                .text_size(TextSize::Sm.font_size())
                                .text_color(muted)
                                .child(rest.trim().to_string()),
                        )
                    });
                let files = match shown_file(&shown.files, self.history_file.as_ref()) {
                    None => div().text_size(TextSize::Xs.font_size()).text_color(muted).child("No file changed.").into_any_element(),
                    Some(file) => div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .gap(px(12.))
                        .child(
                            div()
                                .id("history-tree-column")
                                .flex_none()
                                .w(px(TREE_WIDTH))
                                .h_full()
                                .child(Self::file_tree(&this, "history-tree", ShellView::History, &shown.files, &file.path, true)),
                        )
                        .child(self.one_diff(&this, file))
                        .into_any_element(),
                };
                div()
                    .debug_selector(|| "history-commit".into())
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .size_full()
                    .p(px(16.))
                    .child(head)
                    .child(files)
                    .into_any_element()
            }
        };
        self.code_card(body, cx)
    }
}
