//! The menu on a row of the Files tree, as an editor offers one for a file or a folder, and a mention of it in a session.
use atelier_settings::Location;
use atelier_ui::{
    IconName,
    menu::{self, Entry, Menu, MenuItem, MenuLook, Origin, Tone},
    popover::Popover,
};
use gpui_kit::{AnyElement, Bounds, Context, IntoElement, size};

use super::{structs::Shell, view::ShellView};
use crate::open_project::{OpenProject, tree_edit::TreeEditKind};

/// The folder a new file goes in for a press on `path`: the folder itself, or the one a file is in.
fn folder_for(path: &str, dir: bool) -> String {
    if dir { path.to_string() } else { path.rsplit_once('/').map_or("", |(parent, _)| parent).to_string() }
}

impl Shell {
    /// The menu open on the front project's tree, if there is one.
    pub(super) fn tree_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.view != ShellView::Files || self.settings.is_some() {
            return None;
        }
        let project = self.active()?.clone();
        let (menu, local, sessions) = {
            let p = project.read(cx);
            (p.tree_menu.clone()?, matches!(p.location, Location::Local { .. }), p.sessions.clone())
        };
        let (path, dir) = (menu.path.clone(), menu.dir);
        let parent = folder_for(&path, dir);
        let on = |act: fn(&mut OpenProject, &str, bool, &mut gpui_kit::Window, &mut gpui_kit::Context<OpenProject>)| {
            let (project, path) = (project.clone(), path.clone());
            move |window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                project.update(cx, |p, cx| {
                    p.close_tree_menu(cx);
                    act(p, &path, dir, window, cx);
                })
            }
        };
        let mut entries: Vec<Entry> = Vec::new();
        {
            let (project, parent) = (project.clone(), parent.clone());
            entries.push(MenuItem::new("New file").icon(IconName::Draft).debug_name("tree-new-file").on_select(move |window, cx| {
                let kind = TreeEditKind::NewFile { parent: parent.clone() };
                project.update(cx, |p, cx| p.start_tree_edit(kind, window, cx))
            }).into());
        }
        {
            let (project, parent) = (project.clone(), parent.clone());
            entries.push(MenuItem::new("New folder").icon(IconName::CreateNewFolder).debug_name("tree-new-folder").on_select(move |window, cx| {
                let kind = TreeEditKind::NewFolder { parent: parent.clone() };
                project.update(cx, |p, cx| p.start_tree_edit(kind, window, cx))
            }).into());
        }
        if !path.is_empty() {
            entries.push(Entry::Separator);
            let rename = on(|p, path, _, window, cx| p.start_tree_edit(TreeEditKind::Rename { path: path.to_string() }, window, cx));
            entries.push(MenuItem::new("Rename").icon(IconName::Edit).debug_name("tree-rename").on_select(rename).into());
            let duplicate = on(|p, path, _, _, cx| p.duplicate(path, cx));
            entries.push(MenuItem::new("Duplicate").icon(IconName::Copy).debug_name("tree-duplicate").on_select(duplicate).into());
            let delete = on(|p, path, dir, window, cx| p.delete_asking(path, dir, window, cx));
            entries.push(MenuItem::new("Delete").icon(IconName::Delete).tone(Tone::Destructive).debug_name("tree-delete").on_select(delete).into());
        }
        entries.push(Entry::Separator);
        let full = on(|p, path, _, _, cx| p.copy_path(path, true, cx));
        entries.push(MenuItem::new("Copy path").icon(IconName::Link).debug_name("tree-copy-path").on_select(full).into());
        if !path.is_empty() {
            let relative = on(|p, path, _, _, cx| p.copy_path(path, false, cx));
            entries.push(MenuItem::new("Copy relative path").icon(IconName::Link).debug_name("tree-copy-relative").on_select(relative).into());
        }
        if local {
            let reveal = on(|p, path, _, _, cx| p.reveal_in_files(path, cx));
            let words = if cfg!(target_os = "macos") { "Reveal in Finder" } else { "Show in the file manager" };
            entries.push(MenuItem::new(words).icon(IconName::OpenInNew).debug_name("tree-reveal").on_select(reveal).into());
        }
        let touching = if path.is_empty() { Vec::new() } else { project.read(cx).touched_by(&path, dir, cx) };
        if !touching.is_empty() {
            entries.push(Entry::Separator);
            for session in &touching {
                let title = session.read(cx).shown_title();
                let (project, session, review_path) = (project.clone(), session.clone(), (!dir).then(|| path.clone()));
                entries.push(
                    MenuItem::new(format!("Review changes by {title}"))
                        .icon(IconName::Description)
                        .debug_name("tree-review-session")
                        .on_select(move |_, cx| project.update(cx, |p, cx| p.review_changes_by(&session, review_path.clone(), cx)))
                        .into(),
                );
            }
        }
        if !path.is_empty() && !sessions.is_empty() {
            entries.push(Entry::Separator);
            let branches: Vec<Entry> = sessions
                .iter()
                .map(|session| {
                    let title = session.read(cx).shown_title();
                    let (project, session, path) = (project.clone(), session.clone(), path.clone());
                    MenuItem::new(title)
                        .debug_name("tree-mention-session")
                        .on_select(move |_, cx| {
                            project.update(cx, |p, cx| {
                                p.close_tree_menu(cx);
                                p.mention_in(&session, &path, cx);
                            })
                        })
                        .into()
                })
                .collect();
            entries.push(MenuItem::new("Mention in session").icon(IconName::ChatBubble).debug_name("tree-mention").submenu(branches).into());
        }
        if dir {
            let collapse = on(|p, _, _, _, cx| p.collapse_all(cx));
            entries.push(MenuItem::new("Collapse all folders").icon(IconName::UnfoldMore).debug_name("tree-collapse").on_select(collapse).into());
        }
        let look = MenuLook::PROJECT;
        let height = menu::height_of(look, &entries);
        let close = project.clone();
        // A one-pixel anchor at the press: the panel opens at the pointer, below it when it fits.
        let anchor = Bounds { origin: menu.at, size: size(gpui_kit::px(1.), gpui_kit::px(1.)) };
        Some(
            Popover::new("tree-menu-popover")
                .open(true)
                .anchor(Some(anchor))
                .gap(0.)
                .height(height)
                .keep_focus()
                .on_close(move |_, cx| close.update(cx, |p, cx| p.close_tree_menu(cx)))
                .child(Menu::new("tree-menu-panel", entries).look(look).min_width(200.).origin(Origin::TopLeft))
                .into_any_element(),
        )
    }
}
