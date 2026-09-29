//! One project open in the window: its tree, its git branch, its tabs and their buffers, and the
//! language servers for its files. Everything that touches the project runs on a background thread
//! through [`Project`]; this entity only holds what came back, so switching projects is instant.
//!
//! A file that changes on disk reloads when its tab is clean. When the tab holds unsaved edits, the
//! tab keeps them and says the file changed, with Reload and Keep mine.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use futures_channel::mpsc;
use futures_util::StreamExt;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window,
    base::input::{InputEvent, Position},
    component::input::EditorState,
};
use lathe_editor::{ASK, EditorSession, Elsewhere, Jump, READY};
use lathe_lsp::{Store, Workers};
use lathe_project::{Change, ChangeKind, Project, Watch};
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

/// One open file.
pub struct Buffer {
    pub editor: Entity<EditorState>,
    pub session: Entity<EditorSession>,
    /// The text as it is on disk, as far as this tab knows.
    saved: String,
    pub dirty: bool,
    /// The file changed on disk while this tab held unsaved edits.
    pub changed_on_disk: bool,
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
    pub tabs: Tabs,
    pub buffers: HashMap<String, Buffer>,
    /// Files being read for a tab, so a second click does not read them twice.
    opening: HashSet<String>,
    /// Where the caret goes in a file still being read, after a jump to it.
    caret_at: HashMap<String, Position>,
    _watch: Option<Watch>,
    _tasks: Vec<Task<()>>,
    listing_task: Task<()>,
}

impl OpenProject {
    pub fn new(location: Location, project: Arc<dyn Project>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workers = Arc::new(Workers::new(project.clone(), Store::from_env(), READY, ASK));
        let mut this = Self {
            location,
            project,
            workers,
            listing: Listing::Loading,
            listed_in: None,
            open_folders: HashSet::new(),
            git: Git::Unknown,
            tabs: Tabs::default(),
            buffers: HashMap::new(),
            opening: HashSet::new(),
            caret_at: HashMap::new(),
            _watch: None,
            _tasks: Vec::new(),
            listing_task: Task::ready(()),
        };
        this.relist(cx);
        this.read_git(cx);
        this.watch(window, cx);
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
        self._tasks.push(cx.spawn(async move |this, cx| {
            let git = match asked.await {
                Ok(out) if out.ok() => Git::Branch(out.stdout.trim().to_string().into()),
                _ => Git::None,
            };
            _ = this.update(cx, |this, cx| {
                this.git = git;
                cx.notify();
            });
        }));
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
        self._tasks.push(cx.spawn_in(window, async move |this, cx| {
            while let Some(batch) = rx.next().await {
                _ = this.update_in(cx, |this, window, cx| this.changed(batch, window, cx));
            }
        }));
    }

    /// A batch from the watch: new or removed paths list the tree again; an open file reloads.
    fn changed(&mut self, batch: Vec<Change>, window: &mut Window, cx: &mut Context<Self>) {
        if batch.iter().any(|c| c.kind != ChangeKind::Changed) {
            self.relist(cx);
        }
        for change in batch {
            if change.kind == ChangeKind::Removed {
                continue;
            }
            let Some(buffer) = self.buffers.get_mut(&change.path) else { continue };
            if buffer.dirty {
                buffer.changed_on_disk = true;
                cx.notify();
            } else {
                self.reload(change.path, window, cx);
            }
        }
    }

    /// Reads `path` again and puts its text in its tab, when it differs.
    pub fn reload(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let read = {
            let path = path.clone();
            cx.background_spawn(async move { project.read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()) })
        };
        self._tasks.push(cx.spawn_in(window, async move |this, cx| {
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
        }));
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
        self._tasks.push(cx.spawn_in(window, async move |this, cx| {
            let read = read.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.opening.remove(&path);
                match read {
                    Ok(bytes) => {
                        this.add_buffer(path.clone(), String::from_utf8_lossy(&bytes).into_owned(), window, cx);
                        // The editor scrolls to the caret from its last layout, which a new buffer
                        // gets in its first frame, so the caret moves after it.
                        let this = cx.entity().downgrade();
                        window.on_next_frame(move |window, cx| {
                            this.update(cx, |p, cx| p.place_caret(&path, window, cx)).ok();
                        });
                    }
                    Err(error) => cx.emit(ProjectEvent::Said(format!("Could not open {path}: {error}").into())),
                }
                cx.notify();
            });
        }));
    }

    fn add_buffer(&mut self, path: String, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let editor = beui::CodeEditor::state(&path, text.clone(), window, cx);
        let host = self.project.root().join(&path);
        let workers = self.workers.clone();
        let this = cx.entity().downgrade();
        let elsewhere: Elsewhere = std::rc::Rc::new(move |jump, window, cx| {
            this.update(cx, |p, cx| p.jump(jump, window, cx)).ok();
        });
        let session =
            cx.new(|cx| EditorSession::for_review(workers, editor.clone(), host, beui::RowMap::default(), Some(elsewhere), cx));
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
        self.buffers.insert(path.clone(), Buffer { editor, session, saved: text, dirty: false, changed_on_disk: false, _edits });
        self.tabs.open(&path);
    }

    /// Writes the tab showing, when it has edits.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.tabs.active().map(str::to_string) else { return };
        let Some(buffer) = self.buffers.get(&path) else { return };
        let text = buffer.editor.read(cx).value().to_string();
        let project = self.project.clone();
        let written = {
            let (path, text) = (path.clone(), text.clone());
            cx.background_spawn(async move { project.write(&path, text.as_bytes()) })
        };
        self._tasks.push(cx.spawn(async move |this, cx| {
            let written = written.await;
            _ = this.update(cx, |this, cx| {
                match written {
                    Ok(()) => {
                        if let Some(buffer) = this.buffers.get_mut(&path) {
                            buffer.dirty = buffer.editor.read(cx).value().as_ref() != text;
                            buffer.saved = text;
                            buffer.changed_on_disk = false;
                        }
                        cx.emit(ProjectEvent::Said(format!("Saved {path}").into()));
                    }
                    Err(error) => cx.emit(ProjectEvent::Said(format!("Could not save {path}: {error}").into())),
                }
                cx.notify();
            });
        }));
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
