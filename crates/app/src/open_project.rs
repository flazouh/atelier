//! One project open in the window: its tree, its git branch, its tabs and their buffers, and the
//! language servers for its files. Everything that touches the project runs on a background thread
//! through [`Project`]; this entity only holds what came back, so switching projects is instant.
//!
//! A file that changes on disk reloads when its tab is clean. When the tab holds unsaved edits, the
//! tab keeps them and says the file changed, with Reload and Keep mine. A file deleted on disk says
//! so, with Close and Keep; a kept one holds its text as unsaved, and a save asks before it creates
//! the file again.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use futures_channel::mpsc;
use futures_util::StreamExt;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, PromptLevel, SharedString, Subscription, Task, Window,
    base::input::{InputEvent, Position},
    component::input::EditorState,
};
use lathe_editor::{ASK, EditorSession, Elsewhere, Jump, READY};
use lathe_lsp::{Store, Workers};
use lathe_project::{Change, ChangeKind, Link, Project, Watch};
use lathe_settings::Location;

use crate::{
    tabs::Tabs,
    tree::{ProjectTree, ancestors},
};

/// What `git` says about the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Git {
    /// Not asked yet.
    Unknown,
    /// The folder is not in a git repository.
    None,
    Branch(SharedString),
}

/// The tree as the last listing left it.
#[derive(Clone, Debug)]
pub enum Listing {
    Loading,
    Ready(ProjectTree),
    Failed(SharedString),
}

/// Whether a tab's file is still on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deleted {
    No,
    /// Deleted on disk; the tab asks: Close or Keep.
    Asking,
    /// The reader kept the text; a save asks before it creates the file again.
    Kept,
}

/// One open file.
pub struct Buffer {
    pub editor: Entity<EditorState>,
    pub session: Entity<EditorSession>,
    /// The text as it is on disk, as far as this tab knows.
    saved: String,
    pub dirty: bool,
    /// The file changed on disk while this tab held unsaved edits.
    pub changed_on_disk: bool,
    pub deleted: Deleted,
    _edits: Subscription,
}

pub enum ProjectEvent {
    /// A line for the status line, such as a save that failed.
    Said(SharedString),
}

impl EventEmitter<ProjectEvent> for OpenProject {}

pub struct OpenProject {
    pub location: Location,
    project: Arc<dyn Project>,
    workers: Arc<Workers>,
    pub listing: Listing,
    /// How long the last listing took, for the status line and docs/performance.md.
    pub listed_in: Option<Duration>,
    pub open_folders: HashSet<String>,
    pub git: Git,
    /// Whether the project's host can be reached; always up for a folder on this machine.
    pub link: Link,
    pub tabs: Tabs,
    pub buffers: HashMap<String, Buffer>,
    /// Files being read for a tab, so a second click does not read them twice.
    opening: HashSet<String>,
    /// Where the caret goes in a file still being read, after a jump to it.
    caret_at: HashMap<String, Position>,
    _watch: Option<Watch>,
    /// Hands the watch's batches to this entity, as long as it lives.
    watching: Task<()>,
    /// Hands the link's ups and downs to this entity.
    linking: Task<()>,
    listing_task: Task<()>,
}

impl OpenProject {
    pub fn new(location: Location, project: Arc<dyn Project>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // A remote project's servers run on its host, found by the host's PATH, at the project's root.
        let workers = match project.host() {
            Some(_) => Workers::new(project.clone(), Store::on_host(), READY, ASK).at_project_root(),
            None => Workers::new(project.clone(), Store::from_env(), READY, ASK),
        };
        let workers = Arc::new(workers);
        let mut this = Self {
            location,
            project,
            workers,
            listing: Listing::Loading,
            listed_in: None,
            open_folders: HashSet::new(),
            git: Git::Unknown,
            link: Link::Up,
            tabs: Tabs::default(),
            buffers: HashMap::new(),
            opening: HashSet::new(),
            caret_at: HashMap::new(),
            _watch: None,
            watching: Task::ready(()),
            linking: Task::ready(()),
            listing_task: Task::ready(()),
        };
        this.relist(cx);
        this.read_git(cx);
        this.watch(window, cx);
        this.follow_link(window, cx);
        this
    }

    pub fn name(&self) -> String {
        self.location.name()
    }

    fn relist(&mut self, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let listed = cx.background_spawn(async move {
            let started = Instant::now();
            let listing = project.list().map(ProjectTree::new);
            (listing, started.elapsed())
        });
        // A newer listing replaces an older one still running, which is dropped.
        self.listing_task = cx.spawn(async move |this, cx| {
            let (listing, took) = listed.await;
            _ = this.update(cx, |this, cx| {
                this.listing = match listing {
                    Ok(tree) => Listing::Ready(tree),
                    Err(error) => Listing::Failed(error.to_string().into()),
                };
                this.listed_in = Some(took);
                cx.notify();
            });
        });
    }

    fn read_git(&mut self, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let asked = cx.background_spawn(async move { project.git(&["rev-parse", "--abbrev-ref", "HEAD"]) });
        cx.spawn(async move |this, cx| {
            let git = match asked.await {
                Ok(out) if out.ok() => Git::Branch(out.stdout.trim().to_string().into()),
                _ => Git::None,
            };
            _ = this.update(cx, |this, cx| {
                this.git = git;
                cx.notify();
            });
        }).detach();
    }

    fn watch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded::<Vec<Change>>();
        match self.project.watch(Box::new(move |batch| drop(tx.unbounded_send(batch)))) {
            Ok(watch) => self._watch = Some(watch),
            Err(error) => {
                cx.emit(ProjectEvent::Said(format!("Not watching for changes: {error}").into()));
                return;
            }
        }
        self.watching = cx.spawn_in(window, async move |this, cx| {
            while let Some(batch) = rx.next().await {
                _ = this.update_in(cx, |this, window, cx| this.changed(batch, window, cx));
            }
        });
    }

    fn follow_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded::<Link>();
        self.project.on_link(Box::new(move |link| drop(tx.unbounded_send(link))));
        self.linking = cx.spawn_in(window, async move |this, cx| {
            while let Some(link) = rx.next().await {
                _ = this.update(cx, |this, cx| this.linked(link, cx));
            }
        });
    }

    /// The host dropped or came back. Back, the tree and the branch are read again, since they may
    /// have changed meanwhile, and each open file's language server starts again, since the host's
    /// ended with the connection. The tabs' text never left this machine.
    fn linked(&mut self, link: Link, cx: &mut Context<Self>) {
        let back = link == Link::Up && self.link != Link::Up;
        self.link = link;
        if back {
            self.relist(cx);
            self.read_git(cx);
            let open: Vec<(String, Entity<EditorState>)> = self.buffers.iter().map(|(p, b)| (p.clone(), b.editor.clone())).collect();
            for (path, editor) in open {
                let session = self.session_for(&path, editor, cx);
                if let Some(buffer) = self.buffers.get_mut(&path) {
                    buffer.session = session;
                }
            }
            cx.emit(ProjectEvent::Said("Reconnected".into()));
        }
        cx.notify();
    }

    /// Files being read for a tab, for the tab bar to show them as pending.
    pub fn opening(&self) -> impl Iterator<Item = &String> {
        self.opening.iter()
    }

    /// A batch from the watch: new or removed paths list the tree again; an open file reloads.
    fn changed(&mut self, batch: Vec<Change>, window: &mut Window, cx: &mut Context<Self>) {
        if batch.iter().any(|c| c.kind != ChangeKind::Changed) {
            self.relist(cx);
        }
        for change in batch {
            let Some(buffer) = self.buffers.get_mut(&change.path) else { continue };
            if change.kind == ChangeKind::Removed {
                if buffer.deleted == Deleted::No {
                    buffer.deleted = Deleted::Asking;
                    cx.notify();
                }
                continue;
            }
            // Back on disk, as after a checkout: a change like any other.
            buffer.deleted = Deleted::No;
            if buffer.dirty {
                self.check_disk(change.path, cx);
            } else {
                self.reload(change.path, window, cx);
            }
        }
    }

    /// A dirty tab's file changed: it says so only when the file holds something other than what the
    /// tab last saved, so the watch reporting lathe's own write is no news.
    fn check_disk(&mut self, path: String, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let read = {
            let path = path.clone();
            cx.background_spawn(async move { project.read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()) })
        };
        cx.spawn(async move |this, cx| {
            let Ok(text) = read.await else { return };
            _ = this.update(cx, |this, cx| {
                let Some(buffer) = this.buffers.get_mut(&path) else { return };
                if buffer.dirty && text != buffer.saved {
                    buffer.changed_on_disk = true;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Reads `path` again and puts its text in its tab, when it differs.
    pub fn reload(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let read = {
            let path = path.clone();
            cx.background_spawn(async move { project.read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()) })
        };
        cx.spawn_in(window, async move |this, cx| {
            let Ok(text) = read.await else { return };
            _ = this.update_in(cx, |this, window, cx| {
                let Some(buffer) = this.buffers.get_mut(&path) else { return };
                buffer.changed_on_disk = false;
                buffer.dirty = false;
                if buffer.editor.read(cx).value().as_ref() != text {
                    buffer.editor.update(cx, |e, cx| e.set_value(text.clone(), window, cx));
                }
                buffer.saved = text;
                cx.notify();
            });
        }).detach();
    }

    /// Keeps a deleted file's tab: its text stays, unsaved, until a save creates the file again.
    pub fn keep_deleted(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(buffer) = self.buffers.get_mut(path) {
            buffer.deleted = Deleted::Kept;
            buffer.dirty = true;
            cx.notify();
        }
    }

    /// Keeps the tab's edits over the file's new text; the next save writes them.
    pub fn keep_mine(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(buffer) = self.buffers.get_mut(path) {
            buffer.changed_on_disk = false;
            cx.notify();
        }
    }

    pub fn toggle_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.open_folders.remove(path) {
            self.open_folders.insert(path.to_string());
        }
        cx.notify();
    }

    /// A definition in another file: its tab opens with the caret there. One outside the project,
    /// such as the toolchain's own sources, is named, not opened.
    fn jump(&mut self, jump: Jump, window: &mut Window, cx: &mut Context<Self>) {
        let relative = jump.path.strip_prefix(self.project.root()).ok().and_then(|p| p.to_str()).map(|p| p.replace('\\', "/"));
        let Some(relative) = relative else {
            let name = jump.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            cx.emit(ProjectEvent::Said(format!("Defined in {name} on line {}, outside this project", jump.position.line + 1).into()));
            return;
        };
        self.caret_at.insert(relative.clone(), jump.position);
        self.open_file(&relative, window, cx);
    }

    /// Puts the caret where a jump asked, once the file's tab has its buffer, and gives it focus.
    fn place_caret(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.buffers.get(path) else { return };
        let position = self.caret_at.remove(path);
        buffer.editor.update(cx, |state, cx| {
            if let Some(position) = position {
                state.set_cursor_position(position, window, cx);
            }
            state.focus(window, cx);
        });
    }

    /// Shows `path` in a tab, reading it first when it has none.
    pub fn open_file(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folders.extend(ancestors(path));
        if self.buffers.contains_key(path) {
            self.tabs.open(path);
            self.place_caret(path, window, cx);
            cx.notify();
            return;
        }
        if !self.opening.insert(path.to_string()) {
            return;
        }
        let project = self.project.clone();
        let read = {
            let path = path.to_string();
            cx.background_spawn(async move { project.read(&path) })
        };
        let path = path.to_string();
        cx.spawn_in(window, async move |this, cx| {
            let read = read.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.opening.remove(&path);
                match read {
                    Ok(bytes) => {
                        this.add_buffer(path.clone(), String::from_utf8_lossy(&bytes).into_owned(), window, cx);
                        this.place_caret(&path, window, cx);
                    }
                    Err(error) => cx.emit(ProjectEvent::Said(format!("Could not open {path}: {error}").into())),
                }
                cx.notify();
            });
        }).detach();
    }

    /// The language server session for `path`'s editor, with jumps to other files opening them here.
    fn session_for(&self, path: &str, editor: Entity<EditorState>, cx: &mut Context<Self>) -> Entity<EditorSession> {
        let host = self.project.root().join(path);
        let workers = self.workers.clone();
        let this = cx.entity().downgrade();
        let elsewhere: Elsewhere = std::rc::Rc::new(move |jump, window, cx| {
            this.update(cx, |p, cx| p.jump(jump, window, cx)).ok();
        });
        cx.new(|cx| EditorSession::for_review(workers, editor, host, beui::RowMap::default(), Some(elsewhere), cx))
    }

    fn add_buffer(&mut self, path: String, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let editor = beui::CodeEditor::state(&path, text.clone(), window, cx);
        let session = self.session_for(&path, editor.clone(), cx);
        let key = path.clone();
        let _edits = cx.subscribe(&editor, move |this, editor, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let Some(buffer) = this.buffers.get_mut(&key) else { return };
            let dirty = editor.read(cx).value().as_ref() != buffer.saved;
            if dirty != buffer.dirty {
                buffer.dirty = dirty;
                cx.notify();
            }
        });
        self.buffers.insert(path.clone(), Buffer { editor, session, saved: text, dirty: false, changed_on_disk: false, deleted: Deleted::No, _edits });
        self.tabs.open(&path);
    }

    /// Writes the tab showing; for a file deleted on disk, asks first whether to create it again.
    pub fn save_asking(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.tabs.active().map(str::to_string) else { return };
        if self.buffers.get(&path).is_none_or(|b| b.deleted == Deleted::No) {
            return self.save_path(path, false, cx);
        }
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Create {name} again?"),
            Some("It was deleted on disk. Saving writes it back."),
            &["Create", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                _ = this.update(cx, |this, cx| this.save_path(path, false, cx));
            }
        })
        .detach();
    }

    /// Writes `path`'s tab; with `then_close`, closes it once the write lands.
    fn save_path(&mut self, path: String, then_close: bool, cx: &mut Context<Self>) {
        let Some(buffer) = self.buffers.get(&path) else { return };
        let text = buffer.editor.read(cx).value().to_string();
        let project = self.project.clone();
        let written = {
            let (path, text) = (path.clone(), text.clone());
            cx.background_spawn(async move { project.write(&path, text.as_bytes()) })
        };
        cx.spawn(async move |this, cx| {
            let written = written.await;
            _ = this.update(cx, |this, cx| {
                match written {
                    Ok(()) => {
                        if let Some(buffer) = this.buffers.get_mut(&path) {
                            buffer.dirty = buffer.editor.read(cx).value().as_ref() != text;
                            buffer.saved = text;
                            buffer.changed_on_disk = false;
                            buffer.deleted = Deleted::No;
                        }
                        cx.emit(ProjectEvent::Said(format!("Saved {path}").into()));
                        if then_close {
                            this.close(&path, cx);
                        }
                    }
                    Err(error) => cx.emit(ProjectEvent::Said(format!("Could not save {path}: {error}").into())),
                }
                cx.notify();
            });
        }).detach();
    }

    /// How many tabs hold unsaved edits.
    pub fn unsaved(&self) -> usize {
        self.buffers.values().filter(|b| b.dirty).count()
    }

    /// Closes `path`'s tab. A tab with unsaved edits asks first: Save, Don't Save, or Cancel.
    pub fn close_asking(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.buffers.get(path).is_some_and(|b| b.dirty) {
            self.close(path, cx);
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Save the changes to {name}?"),
            Some("They are lost if you close it without saving."),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            let Ok(answer) = answer.await else { return };
            _ = this.update(cx, |this, cx| match answer {
                0 => this.save_path(path, true, cx),
                1 => this.close(&path, cx),
                _ => {}
            });
        })
        .detach();
    }

    /// Closes `path`'s tab and drops its buffer.
    pub fn close(&mut self, path: &str, cx: &mut Context<Self>) {
        self.tabs.close(path);
        self.buffers.remove(path);
        cx.notify();
    }

    pub fn active_buffer(&self) -> Option<(&str, &Buffer)> {
        let path = self.tabs.active()?;
        Some((path, self.buffers.get(path)?))
    }
}

#[cfg(test)]
mod tests;
