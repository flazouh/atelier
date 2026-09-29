//! The right pane's editor: a tab per open file (its name, a dot while it holds unsaved edits, and a
//! close button on hover), then the file. A file that changed on disk under unsaved edits says so
//! above the text, with Reload and Keep mine.

use beui::{
    CodeEditor,
    button::{Button, ButtonVariant, dot},
    file_icon::FileIcon,
    icon::{Icon, IconName},
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    div, prelude::FluentBuilder, px,
};

use crate::open_project::OpenProject;

pub fn editor_pane(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let p = project.read(cx);
    let active = p.tabs.active().map(str::to_string);
    let tabs = p.tabs.paths().iter().enumerate().map(|(i, path)| {
        let shown = active.as_deref() == Some(path.as_str());
        let dirty = p.buffers.get(path).is_some_and(|b| b.dirty);
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        let group: SharedString = format!("tab-{i}").into();
        let (open, close) = (project.clone(), project.clone());
        let (open_path, close_path) = (path.clone(), path.clone());
        div()
            .id(("tab", i))
            .group(group.clone())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(28.))
            .pl(px(10.))
            .pr(px(4.))
            .rounded(radius::MD)
            .cursor_pointer()
            .text_size(TextSize::Sm.font_size())
            .text_color(if shown { theme.foreground } else { muted })
            .when(shown, |d| d.bg(theme.card_strong))
            .hover(|s| s.text_color(theme.foreground))
            .tooltip(beui::tooltip::Tooltip::text(path.clone()))
            .on_click(move |_, window, cx| open.update(cx, |p, cx| p.open_file(&open_path, window, cx)))
            .child(FileIcon::file(&name).size(px(14.)))
            .child(name)
            // The dot and the close button share one slot, so nothing moves on hover.
            .child(
                div()
                    .id(("tab-close", i))
                    .relative()
                    .flex()
                    .flex_none()
                    .size(px(18.))
                    .items_center()
                    .justify_center()
                    .rounded(radius::MD)
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
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        close.update(cx, |p, cx| p.close(&close_path, cx));
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
            let banner = buffer.changed_on_disk.then(|| {
                let (reload, keep) = (project.clone(), project.clone());
                let (reload_path, keep_path) = (path.to_string(), path.to_string());
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mx(px(8.))
                    .mb(px(6.))
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(radius::LG)
                    .bg(theme.card_strong)
                    .text_size(TextSize::Xs.font_size())
                    .child(Icon::new(IconName::Refresh).size(px(14.)).color(theme.warning))
                    .child(div().flex_1().child("This file changed on disk. Your edits are not saved."))
                    .child(Button::new("reload").label("Reload").variant(ButtonVariant::Secondary).on_click(move |_, window, cx| {
                        reload.update(cx, |p, cx| p.reload(reload_path.clone(), window, cx))
                    }))
                    .child(Button::new("keep-mine").label("Keep mine").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                        keep.update(cx, |p, cx| p.keep_mine(&keep_path, cx))
                    }))
            });
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
        .child(div().id("tabs").flex().flex_none().gap(px(2.)).px(px(6.)).pb(px(6.)).overflow_x_scroll().children(tabs))
        .child(body)
}
