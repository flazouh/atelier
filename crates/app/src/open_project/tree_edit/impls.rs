use std::io;

use atelier_project::FsOp;
use gpui_kit::{AppContext, ClipboardItem, Context, Entity, Pixels, Point, PromptLevel, SharedString, Window, component::input::{InputEvent, InputState}};

use crate::{agent_session::AgentSession, open_project::{Listing, OpenProject, ProjectEvent}};

use super::{
    helpers::{bad_name, copy_name, joined, moved},
    structs::{TreeEdit, TreeMenu},
    types::TreeEditKind,
};

/// The last part of a path.
fn last(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// The folder above `path`; `""` is the project's folder.
fn above(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// What a failed change says, in words.
fn said(what: &str, error: &io::Error) -> SharedString {
    match error.kind() {
        io::ErrorKind::AlreadyExists => "That name is taken already.".into(),
        _ => format!("Could not {what}: {error}").into(),
    }
}

impl OpenProject {
    /// Opens the menu of the row at `path` (or of the empty part of the tree, with `""`) at `at`.
    pub fn open_tree_menu(&mut self, path: String, dir: bool, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.tree_edit = None;
        self.tree_menu = Some(TreeMenu { path, dir, at });
        cx.notify();
    }

    pub fn close_tree_menu(&mut self, cx: &mut Context<Self>) {
        if self.tree_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Whether the tree lists `path`.
    fn lists(&self, path: &str) -> bool {
        matches!(&self.listing, Listing::Ready(tree) if tree.has(path))
    }

    /// Starts a name in the tree: a row with an input for a new file or folder, or in place of the name of the row to rename.
    /// Enter keeps it, and leaving the input drops it.
    pub fn start_tree_edit(&mut self, kind: TreeEditKind, window: &mut Window, cx: &mut Context<Self>) {
        self.tree_menu = None;
        let start = match &kind {
            TreeEditKind::Rename { path } => last(path).to_string(),
            TreeEditKind::NewFile { parent } | TreeEditKind::NewFolder { parent } => {
                if !parent.is_empty() {
                    self.reveal_folder(parent, cx);
                }
                String::new()
            }
        };
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(start, window, cx);
            state
        });
        input.update(cx, |i, cx| i.focus(window, cx));
        let events = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
            InputEvent::PressEnter { .. } => this.commit_tree_edit(cx),
            InputEvent::Blur => this.cancel_tree_edit(cx),
            _ => {}
        });
        self.tree_edit = Some(TreeEdit { kind, input, _events: events });
        cx.notify();
    }

    pub fn cancel_tree_edit(&mut self, cx: &mut Context<Self>) {
        if self.tree_edit.take().is_some() {
            cx.notify();
        }
    }

    /// Does what the typed name asks. A name that cannot be used says why and stays in the input.
    pub fn commit_tree_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.tree_edit.take() else { return };
        let name = edit.input.read(cx).value().trim().to_string();
        let target = match &edit.kind {
            TreeEditKind::NewFile { parent } | TreeEditKind::NewFolder { parent } => joined(parent, &name),
            TreeEditKind::Rename { path } => joined(above(path), &name),
        };
        let refusal = match &edit.kind {
            TreeEditKind::Rename { path } if *path == target => {
                cx.notify();
                return;
            }
            _ => bad_name(&name).map(SharedString::from).or_else(|| self.lists(&target).then(|| "That name is taken already.".into())),
        };
        if let Some(why) = refusal {
            cx.emit(ProjectEvent::Said(why));
            self.tree_edit = Some(edit);
            cx.notify();
            return;
        }
        match edit.kind.clone() {
            TreeEditKind::NewFile { .. } => {
                let open = target.clone();
                self.run_op(FsOp::NewFile { path: target }, "make the file", move |_, cx| cx.emit(ProjectEvent::Open(open)), cx);
            }
            TreeEditKind::NewFolder { .. } => {
                let shown = target.clone();
                self.run_op(FsOp::NewFolder { path: target }, "make the folder", move |this, cx| this.reveal_folder(&shown, cx), cx);
            }
            TreeEditKind::Rename { path } => self.rename(path, target, cx),
        }
        cx.notify();
    }

    /// Moves `from` to `to`. The tabs of what moved close and open again at the new place; one with unsaved edits stops it.
    fn rename(&mut self, from: String, to: String, cx: &mut Context<Self>) {
        if self.buffers.iter().any(|(path, buffer)| buffer.dirty && moved(path, &from, "").is_some()) {
            cx.emit(ProjectEvent::Said(format!("Save the open files in {} first.", last(&from)).into()));
            return;
        }
        let (source, target) = (from.clone(), to.clone());
        self.run_op(FsOp::Rename { from, to }, "rename", move |this, cx| {
            let active = this.tabs.active().map(str::to_string);
            let moving: Vec<String> = this.tabs.paths().iter().filter(|p| moved(p, &source, &target).is_some()).cloned().collect();
            for path in &moving {
                this.close(path, cx);
            }
            // The folders that were open stay open at their new place.
            let reopened: Vec<String> = this.open_folders.iter().filter_map(|f| moved(f, &source, &target)).collect();
            this.open_folders.retain(|f| moved(f, &source, &target).is_none());
            this.open_folders.extend(reopened);
            // The one that showed opens last, so it shows again.
            let mut order: Vec<&String> = moving.iter().filter(|p| Some(p.as_str()) != active.as_deref()).collect();
            order.extend(moving.iter().filter(|p| Some(p.as_str()) == active.as_deref()));
            for path in order {
                if let Some(new) = moved(path, &source, &target) {
                    cx.emit(ProjectEvent::Open(new));
                }
            }
        }, cx);
    }

    /// Copies `path` beside itself, under a name that is free.
    pub fn duplicate(&mut self, path: &str, cx: &mut Context<Self>) {
        let Listing::Ready(tree) = &self.listing else { return };
        let to = copy_name(|p| tree.has(p), path);
        self.run_op(FsOp::Copy { from: path.to_string(), to }, "copy", |_, _| {}, cx);
    }

    /// Asks, then removes `path` from the disk and closes its tabs.
    pub fn delete_asking(&mut self, path: &str, dir: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.tree_menu = None;
        let name = last(path);
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {name}?"),
            Some(if dir { "It and everything in it are deleted from the disk. This cannot be undone." } else { "It is deleted from the disk. This cannot be undone." }),
            &["Delete", "Cancel"],
            cx,
        );
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            if !matches!(answer.await, Ok(0)) {
                return;
            }
            _ = this.update(cx, |this, cx| {
                let gone = path.clone();
                this.run_op(FsOp::Delete { path }, "delete", move |this, cx| {
                    let closing: Vec<String> = this.tabs.paths().iter().filter(|p| moved(p, &gone, "").is_some()).cloned().collect();
                    for path in closing {
                        this.close(&path, cx);
                    }
                    this.open_folders.retain(|f| moved(f, &gone, "").is_none());
                }, cx);
            });
        })
        .detach();
    }

    /// Puts the path on the clipboard: from the project's folder, or the host's own full path.
    pub fn copy_path(&mut self, path: &str, full: bool, cx: &mut Context<Self>) {
        self.tree_menu = None;
        let text = if full { self.project.root().join(path).display().to_string() } else { path.to_string() };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        cx.notify();
    }

    /// Shows `path` in the system's file manager. Only a folder on this machine has one.
    pub fn reveal_in_files(&mut self, path: &str, cx: &mut Context<Self>) {
        self.tree_menu = None;
        cx.reveal_path(&self.project.root().join(path));
        cx.notify();
    }

    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.tree_menu = None;
        self.open_folders.clear();
        cx.notify();
    }

    /// Puts a mention of `path` in the composer of `session`, to be there when the reader goes back to it.
    pub fn mention_in(&mut self, session: &Entity<AgentSession>, path: &str, cx: &mut Context<Self>) {
        self.tree_menu = None;
        let title = session.update(cx, |s, cx| {
            s.mention_file(path, cx);
            s.shown_title()
        });
        cx.emit(ProjectEvent::Said(format!("{} is in the message to {title}", last(path)).into()));
        cx.notify();
    }

    /// Does `op` off the UI thread; on success the tree is read again and `then` runs.
    fn run_op(&mut self, op: FsOp, what: &'static str, then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let done = cx.background_spawn(async move { project.apply(&op) });
        cx.spawn(async move |this, cx| {
            let result = done.await;
            _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.relist(cx);
                    then(this, cx);
                    cx.notify();
                }
                Err(error) => cx.emit(ProjectEvent::Said(said(what, &error))),
            });
        })
        .detach();
    }
}
