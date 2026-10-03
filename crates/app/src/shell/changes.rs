//! The Code lens's Changes view: what the project's checkout holds uncommitted. The sidebar lists the files
//! under the branch, then the repository's other worktrees; the main card holds every file's diff, or a
//! session's review while one is open.

use atelier_ui::file_icon::FileIcon;
use atelier_ui::scale::px;
use atelier_ui::theme::{ActiveTheme, radius};
use atelier_ui::typography::TextSize;
use atelier_ui::worktree_list::note_colour;
use atelier_ui::file_diff::diff_stats;
use atelier_ui::{Icon, IconName};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Entity, FontWeight, SharedString, div};

use super::history::{diff_cards, note};
use super::structs::Shell;
use crate::history::Read;
use crate::open_project::OpenProject;

/// The folder part of `path` and its file name.
fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

impl Shell {
    /// The Changes view's sidebar: the branch and its uncommitted files, a row each, then the other worktrees.
    pub(super) fn changes_list(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        if project.read(cx).uncommitted.is_none() {
            project.update(cx, |p, cx| p.load_uncommitted(cx));
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let p = project.read(cx);
        let reviewing = p.review.is_some();
        let files = match &p.uncommitted {
            Some(Read::Ready(files)) => Some(files.as_slice()),
            _ => None,
        };
        let count = files.map(<[_]>::len);
        let heading = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(28.))
            .px(px(8.))
            .text_size(TextSize::Sm.font_size())
            .child(Icon::new(IconName::Commit).size(px(14.)).color(muted))
            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(p.git.branch().cloned().unwrap_or_else(|| "No branch".into())))
            .children(count.filter(|n| *n > 0).map(|n| div().text_size(TextSize::Xs.font_size()).text_color(theme.warning).child(format!("{n} uncommitted"))));
        let list: AnyElement = match (&p.uncommitted, files) {
            (_, Some([])) => div().px(px(8.)).py(px(4.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("Nothing uncommitted.").into_any_element(),
            (_, Some(files)) => div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .children(files.iter().enumerate().map(|(i, f)| {
                    let (dir, name) = split_path(&f.path);
                    let (added, removed) = diff_stats(&f.lines);
                    let this = cx.entity().downgrade();
                    let project = project.downgrade();
                    div()
                        .id(("change-row", i))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(26.))
                        .px(px(8.))
                        .rounded(radius::md())
                        .cursor_pointer()
                        .text_size(TextSize::Sm.font_size())
                        .hover(|s| s.bg(theme.card_strong.opacity(0.6)))
                        .child(FileIcon::file(&f.path).size(px(14.)))
                        .child(div().flex_none().max_w(gpui_kit::relative(0.7)).truncate().child(name.to_string()))
                        .child(div().flex_1().min_w_0().truncate().text_size(TextSize::Xs.font_size()).text_color(muted).child(dir.to_string()))
                        .when(added > 0, |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.diff_color(true)).child(format!("+{added}"))))
                        .when(removed > 0, |d| d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.diff_color(false)).child(format!("\u{2212}{removed}"))))
                        .on_click(move |_, _, cx| {
                            // A session's review gives the card back to the checkout's changes first.
                            drop(project.update(cx, |p, cx| p.close_review(cx)));
                            drop(this.update(cx, |s, cx| {
                                s.changes_scroll.scroll_to_top_of_item(i + 1);
                                cx.notify();
                            }));
                        })
                }))
                .into_any_element(),
            (Some(Read::Failed(why)), _) => div().px(px(8.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(why.clone()).into_any_element(),
            _ => div().px(px(8.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("Reading the changes…").into_any_element(),
        };
        let rows = p.worktree_rows();
        let others: Vec<AnyElement> = rows
            .iter()
            .filter(|_| rows.len() > 1)
            .map(|w| {
                let selector = format!("worktree-{}", w.path);
                div()
                    .debug_selector(move || selector.clone())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(26.))
                    .px(px(8.))
                    .text_size(TextSize::Sm.font_size())
                    .child(Icon::new(IconName::Commit).size(px(14.)).color(muted))
                    .child(div().flex_none().max_w(gpui_kit::relative(0.5)).truncate().child(w.branch.clone().unwrap_or_else(|| "detached".into())))
                    .child(div().flex_1().min_w_0().truncate().text_size(TextSize::Xs.font_size()).text_color(muted).child(if w.main { "main checkout".into() } else { w.folder.clone() }))
                    .children(w.notes.iter().map(|n| div().flex_none().text_size(TextSize::Xs.font_size()).text_color(note_colour(n.tone, &theme)).child(n.words.clone())))
                    .into_any_element()
            })
            .collect();
        div()
            .id("changes-list")
            .debug_selector(|| "changes-list".into())
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .p(px(6.))
            .child(heading)
            .when(reviewing, |d| {
                d.child(div().px(px(8.)).pb(px(4.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("Showing a session's review. Press a file to see the checkout."))
            })
            .child(list)
            .when(!others.is_empty(), |d| {
                d.child(div().debug_selector(|| "worktrees".into()).px(px(8.)).pt(px(14.)).pb(px(4.)).text_size(TextSize::Xs.font_size()).text_color(muted).child("Worktrees")).children(others)
            })
            .into_any_element()
    }

    /// The Changes view's main area: a session's review while one is open, else every uncommitted file's diff.
    pub(super) fn changes_main(&self, project: &Entity<OpenProject>, cx: &mut Context<Self>) -> AnyElement {
        let p = project.read(cx);
        if let Some((pane, _)) = p.review.as_ref() {
            return div().debug_selector(|| "review-in-place".into()).size_full().pl(px(atelier_ui::panel_layout::GAP)).pr(px(8.)).pb(px(8.)).child(pane.clone()).into_any_element();
        }
        let muted = cx.theme().muted_foreground;
        let body = match &p.uncommitted {
            Some(Read::Ready(files)) if files.is_empty() => note("Nothing uncommitted: the checkout is at its last commit.", muted),
            Some(Read::Ready(files)) => {
                let branch: SharedString = p.git.branch().cloned().unwrap_or_else(|| "this checkout".into());
                let head = div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pb(px(8.))
                    .child(div().text_size(TextSize::Base.font_size()).font_weight(FontWeight::MEDIUM).child(format!("Uncommitted on {branch}")))
                    .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child(match files.len() {
                        1 => "1 file".to_string(),
                        n => format!("{n} files"),
                    }));
                div()
                    .id("changes-diffs")
                    .debug_selector(|| "changes-diffs".into())
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.changes_scroll)
                    .p(px(16.))
                    .child(head)
                    .children(diff_cards("change-file", files))
                    .into_any_element()
            }
            Some(Read::Failed(why)) => note(why.clone(), muted),
            _ => note("Reading the changes…", muted),
        };
        self.code_card(body, cx)
    }
}
