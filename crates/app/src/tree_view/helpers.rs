use std::rc::Rc;

use atelier_ui::{
    file_icon::FileIcon,
    icon::{Icon, IconName},
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    App,
    Entity,
    InteractiveElement,
    IntoElement,
    MouseButton,
    ParentElement,
    SharedString,
    StatefulInteractiveElement,
    Styled,
    div,
    prelude::FluentBuilder,
    uniform_list,
};
use atelier_ui::scale::px;

use gpui_kit::component::input::Input;
use crate::{
    open_project::{Listing, OpenProject, tree_edit::TreeEditKind},
    tree::Row,
};
use super::types::{ROW, STEP};

pub fn tree_view(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let p = project.read(cx);
    let note = |words: &str| div().px(px(12.)).py(px(8.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(SharedString::from(words.to_string()));
    let tree = match &p.listing {
        Listing::Loading => return note("Listing the files…").into_any_element(),
        Listing::Failed(error) => return note(&format!("Could not list the files: {error}")).into_any_element(),
        Listing::Ready(tree) if tree.is_empty() && p.tree_edit.is_none() => return note("This folder is empty.").into_any_element(),
        Listing::Ready(tree) => tree,
    };
    let mut rows = tree.rows(&p.open_folders);
    // A name being typed has a row of its own: in place of the row being renamed, or first in the folder it is made in.
    let edit = p.tree_edit.as_ref().map(|e| (e.kind.clone(), e.input.clone()));
    let edit_at = edit.as_ref().map(|(kind, _)| match kind {
        TreeEditKind::Rename { path } => rows.iter().position(|r| r.path == *path).unwrap_or(0),
        TreeEditKind::NewFile { parent } | TreeEditKind::NewFolder { parent } => {
            let at = rows.iter().position(|r| r.dir && r.path == *parent);
            let (index, depth) = at.map_or((0, 0), |i| (i + 1, rows[i].depth + 1));
            rows.insert(index, Row { path: String::new(), name: String::new(), depth, dir: matches!(kind, TreeEditKind::NewFolder { .. }), open: false });
            index
        }
    });
    let rows = Rc::new(rows);
    let active = p.tabs.active().map(str::to_string);
    let menu_on = p.tree_menu.as_ref().map(|m| m.path.clone());
    let project = project.clone();
    uniform_list("project-tree", rows.len(), move |range, _, _| {
        range
            .map(|i| {
                let row = &rows[i];
                // The slot is the list item: it takes the list's whole width, and the row fills it inside its margin.
                let slot = div().w_full().px(px(6.));
                if let (Some(at), Some((_, input))) = (edit_at, &edit)
                    && at == i
                {
                    return slot
                        .child(
                            div()
                                .debug_selector(|| "tree-edit-row".into())
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .w_full()
                                .h(px(ROW))
                                .pl(px(8. + STEP * row.depth as f32))
                                .pr(px(8.))
                                .rounded(radius::md())
                                .bg(theme.card_strong)
                                .text_size(TextSize::Sm.font_size())
                                .child(div().w(px(12.)).flex_none())
                                .child(if row.dir { FileIcon::folder(&row.name, false).size(px(14.)) } else { FileIcon::file(&row.name).size(px(14.)) })
                                .child(div().flex_1().min_w_0().child(Input::new(input).appearance(false).text_size(TextSize::Sm.font_size()))),
                        )
                        .into_any_element();
                }
                let shown = active.as_deref() == Some(row.path.as_str());
                let held = menu_on.as_deref() == Some(row.path.as_str());
                let (path, dir) = (row.path.clone(), row.dir);
                let (project, on_press, on_more) = (project.clone(), project.clone(), project.clone());
                let menu_path = path.clone();
                let (more, more_path) = (on_more.clone(), path.clone());
                let group: gpui_kit::SharedString = format!("tree-group-{i}").into();
                slot.child(
                    div()
                        .id(("tree-row", i))
                        .debug_selector({
                            let path = path.clone();
                            move || format!("tree-row-{path}")
                        })
                        .group(group.clone())
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(ROW))
                        .pl(px(8. + STEP * row.depth as f32))
                        .pr(px(4.))
                        .rounded(radius::md())
                        .cursor_pointer()
                        .text_size(TextSize::Sm.font_size())
                        .when(shown || held, |d| d.bg(theme.muted_hover()))
                        .hover(|s| s.bg(theme.muted_hover()))
                        // A press with the other button opens the row's menu where the pointer is.
                        .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                            cx.stop_propagation();
                            let (path, at) = (menu_path.clone(), event.position);
                            on_press.update(cx, |p, cx| p.open_tree_menu(path, dir, at, cx));
                        })
                        // A file goes through the shell, which opens it and brings the editor to the front.
                        .on_click(move |_, _, cx| {
                            project.update(cx, |p, cx| {
                                if dir {
                                    p.toggle_folder(&path, cx)
                                } else {
                                    cx.emit(crate::open_project::ProjectEvent::Open(path.clone()))
                                }
                            })
                        })
                        .child(div().w(px(12.)).flex_none().when(row.dir, |d| {
                            d.child(Icon::new(if row.open { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(12.)).color(muted))
                        }))
                        .child(if row.dir { FileIcon::folder(&row.name, row.open).size(px(14.)) } else { FileIcon::file(&row.name).size(px(14.)) })
                        .child(div().flex_1().min_w_0().truncate().child(row.name.clone()))
                        // The row's own menu, behind a button that shows while the pointer is on the row or its menu is open.
                        .child(
                            div()
                                .id(("tree-more", i))
                                .debug_selector({
                                    let path = more_path.clone();
                                    move || format!("tree-more-{path}")
                                })
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(20.))
                                .rounded(radius::md())
                                .text_color(muted)
                                .when(!held, |d| d.invisible().group_hover(group, |s| s.visible()))
                                .hover(|s| s.bg(theme.card_strong).text_color(theme.foreground))
                                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                    cx.stop_propagation();
                                    let (path, at) = (more_path.clone(), event.position);
                                    more.update(cx, |p, cx| p.open_tree_menu(path, dir, at, cx));
                                })
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(Icon::new(IconName::MoreHoriz).size(px(14.))),
                        ),
                )
                .into_any_element()
            })
            .collect()
    })
    .size_full()
    .into_any_element()
}
