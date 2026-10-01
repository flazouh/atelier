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
    ParentElement,
    SharedString,
    StatefulInteractiveElement,
    Styled,
    div,
    prelude::FluentBuilder,
    uniform_list,
};
use atelier_ui::scale::px;

use crate::open_project::{Listing, OpenProject};
use super::types::{ROW, STEP};

pub fn tree_view(project: &Entity<OpenProject>, cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let p = project.read(cx);
    let note = |words: &str| div().px(px(12.)).py(px(8.)).text_size(TextSize::Xs.font_size()).text_color(muted).child(SharedString::from(words.to_string()));
    let tree = match &p.listing {
        Listing::Loading => return note("Listing the files…").into_any_element(),
        Listing::Failed(error) => return note(&format!("Could not list the files: {error}")).into_any_element(),
        Listing::Ready(tree) if tree.is_empty() => return note("This folder is empty.").into_any_element(),
        Listing::Ready(tree) => tree,
    };
    let rows = Rc::new(tree.rows(&p.open_folders));
    let active = p.tabs.active().map(str::to_string);
    let project = project.clone();
    uniform_list("project-tree", rows.len(), move |range, _, _| {
        range
            .map(|i| {
                let row = &rows[i];
                let shown = active.as_deref() == Some(row.path.as_str());
                let (path, dir) = (row.path.clone(), row.dir);
                let project = project.clone();
                div()
                    .id(("tree-row", i))
                    .debug_selector({
                        let path = path.clone();
                        move || format!("tree-row-{path}")
                    })
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(ROW))
                    .pl(px(8. + STEP * row.depth as f32))
                    .pr(px(8.))
                    .mx(px(6.))
                    .rounded(radius::md())
                    .cursor_pointer()
                    .text_size(TextSize::Sm.font_size())
                    .when(shown, |d| d.bg(theme.muted_hover()))
                    .hover(|s| s.bg(theme.muted_hover()))
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
                    .child(
                        div().w(px(12.)).flex_none().when(row.dir, |d| {
                            d.child(Icon::new(if row.open { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(12.)).color(muted))
                        }),
                    )
                    .child(if row.dir { FileIcon::folder(&row.name, row.open).size(px(14.)) } else { FileIcon::file(&row.name).size(px(14.)) })
                    .child(div().min_w_0().truncate().child(row.name.clone()))
            })
            .collect()
    })
    .size_full()
    .into_any_element()
}
