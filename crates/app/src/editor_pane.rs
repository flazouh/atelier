//! The right pane's editor: a tab per open file (its name, a dot while it holds unsaved edits, and a
//! close button on hover), then the file. A file that changed on disk under unsaved edits says so
//! above the text, with Reload and Keep mine.

use atelier_ui::{
    Breadcrumb, CodeEditor, Crumb, Tab, Tabs,
    button::{Button, ButtonVariant, dot},
    file_icon::FileIcon,
    icon::{Icon, IconName},
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    div, prelude::FluentBuilder, 
};
use atelier_ui::scale::px;

use crate::open_project::{Deleted, OpenProject};

/// The tab strip: a tab per open file, and a pending tab for each file being read. The Files view draws it in the title bar,
/// and the right pane draws it above the file.
pub fn editor_tabs(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let p = project.read(cx);
    let active = p.tabs.active().map(str::to_string);
    let paths: Vec<String> = p.tabs.paths().to_vec();
    let selected = active.as_deref().and_then(|a| paths.iter().position(|p| p == a));
    let tabs = paths.iter().enumerate().map(|(i, path)| {
        let dirty = p.buffers.get(path).is_some_and(|b| b.dirty);
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        let group: SharedString = format!("tab-{i}").into();
        let close = project.clone();
        let close_path = path.clone();
        Tab::new(name.clone())
            .group(group.clone())
            .tooltip(path.clone())
            .debug_name(format!("editor-tab-{i}"))
            .leading(FileIcon::file(&name).size(px(14.)))
            // The dot and the close button share one slot, so nothing moves on hover.
            .trailing(
                div()
                    .id(("tab-close", i))
                    .relative()
                    .flex()
                    .flex_none()
                    .size(px(18.))
                    .items_center()
                    .justify_center()
                    .rounded(radius::md())
                    .when(dirty, |d| d.child(div().group_hover(group.clone(), |s| s.invisible()).child(dot(muted))))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .invisible()
                            .group_hover(group, |s| s.visible())
                            .child(Icon::new(IconName::Close).size(px(12.))),
                    )
                    .hover(|s| s.bg(theme.muted_hover()))
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        close.update(cx, |p, cx| p.close_asking(&close_path, window, cx));
                    }),
            )
    });
    // A file still being read shows as a pending tab, so a slow host is seen to be working.
    let opening = p.opening().enumerate().map(|(i, path)| {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        Tab::new(name).pending(true).leading(atelier_ui::spinner::Spinner::new(("opening", i)).size(px(12.)).color(muted))
    });
    let open = project.clone();
    Tabs::new("editor-tabs", atelier_ui::design_preview::tab_variant(atelier_ui::design_preview::tabs(cx)), tabs.chain(opening), selected).on_select(move |i, window, cx| {
        if let Some(path) = paths.get(i) {
            open.update(cx, |p, cx| p.open_file(path, window, cx));
        }
    })
}

pub fn editor_pane(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    editor_view(project, cx, true)
}

/// The editor under a tab strip that is drawn elsewhere: the crumbs and the file.
pub fn editor_below_tabs(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    editor_view(project, cx, false)
}

fn editor_view(project: &Entity<OpenProject>, cx: &App, with_tabs: bool) -> impl IntoElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let p = project.read(cx);
    let active = p.tabs.active().map(str::to_string);
    let crumbs = active.as_deref().map(|path| {
        let parts: Vec<String> = path.split('/').map(str::to_string).collect();
        let reveal = project.clone();
        let folders = parts.clone();
        div().flex().flex_none().px(px(14.)).child(
            Breadcrumb::new("editor-crumbs", parts.iter().map(|p| Crumb::new(p.clone()))).debug_name("editor-crumb").on_press(move |i, _, cx| {
                reveal.update(cx, |p, cx| p.reveal_folder(&folders[..=i].join("/"), cx));
            }),
        )
    });
    let body = match p.active_buffer() {
        None => div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(div().text_size(TextSize::Sm.font_size()).child("No file open"))
            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("Pick one in the tree on the left."))
            .into_any_element(),
        Some((path, buffer)) => {
            let banner = if buffer.deleted == Deleted::Asking {
                let (close, keep) = (project.clone(), project.clone());
                let (close_path, keep_path) = (path.to_string(), path.to_string());
                Some(banner(
                    IconName::Close,
                    theme.danger,
                    "This file was deleted on disk.",
                    Button::new("close-deleted").label("Close").variant(ButtonVariant::Secondary).on_click(move |_, _, cx| {
                        close.update(cx, |p, cx| p.close(&close_path, cx))
                    }),
                    Button::new("keep-deleted").label("Keep").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                        keep.update(cx, |p, cx| p.keep_deleted(&keep_path, cx))
                    }),
                    &theme,
                ))
            } else if buffer.changed_on_disk {
                let (reload, keep) = (project.clone(), project.clone());
                let (reload_path, keep_path) = (path.to_string(), path.to_string());
                Some(banner(
                    IconName::Refresh,
                    theme.warning,
                    "This file changed on disk. Your edits are not saved.",
                    Button::new("reload").label("Reload").variant(ButtonVariant::Secondary).on_click(move |_, window, cx| {
                        reload.update(cx, |p, cx| p.reload(reload_path.clone(), window, cx))
                    }),
                    Button::new("keep-mine").label("Keep mine").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                        keep.update(cx, |p, cx| p.keep_mine(&keep_path, cx))
                    }),
                    &theme,
                ))
            } else {
                None
            };
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .children(banner)
                .child(div().flex_1().min_h_0().child(CodeEditor::new(&buffer.editor).on_card(true).fill(true)))
                .into_any_element()
        }
    };
    div()
        .flex()
        .flex_col()
        .size_full()
        .children(with_tabs.then(|| div().flex().flex_none().px(px(6.)).child(editor_tabs(project, cx))))
        .children(crumbs)
        .child(body)
}

/// A line above the text about the file on disk, with its two answers.
fn banner(icon: IconName, tone: gpui_kit::Hsla, words: &'static str, first: Button, second: Button, theme: &atelier_ui::Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .mx(px(8.))
        .mb(px(6.))
        .px(px(12.))
        .py(px(6.))
        .rounded(radius::lg())
        .bg(theme.card_strong)
        .text_size(TextSize::Xs.font_size())
        .child(Icon::new(icon).size(px(14.)).color(tone))
        .child(div().flex_1().child(words))
        .child(first)
        .child(second)
}
