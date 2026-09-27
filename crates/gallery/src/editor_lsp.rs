//! A file's language server, wired into the editor the way Zed does it: hold ⌘ (ctrl on Linux) over
//! a symbol to underline it and click to go to its definition, or to its uses when it is the
//! definition; F12 does the same from the caret and ⇧F12 lists the uses; rest the pointer on a symbol
//! for its hover card; problems are checked on open and again whenever typing pauses.
//!
//! Nothing here names a language: the file's path picks the server from `lathe_lsp::servers`, and
//! [`LspWorker`] runs it on a thread of its own, so no answer ever holds the window.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use futures_channel::oneshot;
use gpui_kit::{
    App, AppContext, Context, Entity, SharedString, Subscription, Task, Window,
    base::input::{self, DefinitionProvider, HoverProvider, InputEvent, Rope, RopeExt},
    component::input::EditorState,
};
use lathe_lsp::{Doc, Found, LspError, LspWorker, Navigation, Reply, Target, Workers, canonical, client::uri_to_path};
use lsp_types::{Diagnostic, Hover, LocationLink, Position, ShowDocumentParams, Uri};

/// How long one question may take. Servers answer in milliseconds once they have indexed.
const ASK: Duration = Duration::from_secs(20);
/// How long a server may take to start and shake hands.
const READY: Duration = Duration::from_secs(60);
/// How long typing must pause before the buffer is checked again. docs/code-editor.md sets 150ms.
const RECHECK_AFTER: Duration = Duration::from_millis(150);
/// The key a Zed user holds to follow a symbol.
const SECONDARY: &str = if cfg!(target_os = "macos") { "⌘" } else { "ctrl" };

/// The newest answer to a Cmd-hover, Cmd-click or F12, kept so a click can tell one place to jump to
/// from several to list. The provider writes it off the main thread.
type LastNavigation = Arc<Mutex<Option<Navigation>>>;

/// Every server the gallery runs: one per (server, project), shared by all its tabs.
fn workers() -> &'static Workers {
    static WORKERS: OnceLock<Workers> = OnceLock::new();
    WORKERS.get_or_init(|| Workers::new(READY, ASK))
}

/// One file's language server: it starts the server, hands the editor its definitions and hover
/// cards, keeps the problems current, lists references, and says what it is doing.
pub struct EditorSession {
    editor: Entity<EditorState>,
    path: PathBuf,
    worker: Option<LspWorker>,
    /// The server's state, or where the last definition outside this file is.
    server: SharedString,
    /// What the last check found.
    problems: SharedString,
    /// The uses of a symbol, when there is more than one to choose from.
    references: Vec<Target>,
    last: LastNavigation,
    checking: Task<()>,
    recheck: Task<()>,
    finding: Task<()>,
    _start: Task<()>,
    _edits: Subscription,
}

impl EditorSession {
    /// Starts the server for `path`, whose text `editor` holds, and attaches it once it is ready.
    pub fn new(editor: Entity<EditorState>, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let path = canonical(&path);
        let started = start(path.clone());
        let _start = cx.spawn(async move |this, cx| {
            let started = started.await.unwrap_or_else(|_| Err("the server thread stopped".into()));
            _ = this.update(cx, |this, cx| this.started(started, cx));
        });
        let _edits = cx.subscribe(&editor, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.references.clear();
                this.schedule_recheck(cx);
            }
        });
        Self {
            editor,
            path,
            worker: None,
            server: "starting the language server".into(),
            problems: SharedString::default(),
            references: Vec::new(),
            last: LastNavigation::default(),
            checking: Task::ready(()),
            recheck: Task::ready(()),
            finding: Task::ready(()),
            _start,
            _edits,
        }
    }

    fn started(&mut self, started: Result<LspWorker, String>, cx: &mut Context<Self>) {
        let worker = match started {
            Ok(worker) => worker,
            Err(message) => {
                self.server = message.into();
                return cx.notify();
            }
        };
        let session = cx.entity().downgrade();
        let last = self.last.clone();
        let here = self.path.clone();
        let show: ShowDocument = Rc::new(move |params: &ShowDocumentParams, cx: &mut App| {
            // gpui-base jumps from its own cache of the Cmd-hover answer. `last` is only trusted when
            // it is that same answer, so a newer hover elsewhere can never decide this click.
            let clicked = uri_to_path(&params.uri).map(|p| canonical(&p));
            let start = params.selection.map(|r| r.start);
            let answer = last.lock().ok().and_then(|last| last.clone()).filter(|n| {
                n.targets.first().is_some_and(|t| t.path == clicked && Some(t.range.start) == start)
            });
            if let Some(navigation) = answer.clone().filter(|n| n.found == Found::References && n.targets.len() > 1) {
                _ = session.update(cx, |this, cx| this.show_references(navigation.targets, cx));
                return true;
            }
            if clicked.as_deref() == Some(here.as_path()) {
                return false;
            }
            let line = start.map_or(1, |p| p.line + 1);
            let verb = match answer.map(|n| n.found) {
                Some(Found::References) => "used in",
                _ => "defined in",
            };
            let text = format!("{verb} {} on line {line}", file_name(&params.uri));
            _ = session.update(cx, |this, cx| {
                this.server = text.into();
                cx.notify();
            });
            true
        });
        attach(&self.editor, &self.path, worker.clone(), self.last.clone(), show, cx);
        self.server = format!("{} is ready: hold {SECONDARY} and click a symbol, or press F12", worker.name()).into();
        self.worker = Some(worker);
        self.check(cx);
    }

    /// What the status line says: the server's state, then what the last check found.
    pub fn status(&self) -> SharedString {
        match self.problems.is_empty() {
            true => self.server.clone(),
            false => format!("{} · {}", self.server, self.problems).into(),
        }
    }

    /// The uses to list under the editor. Empty when there is nothing to choose between.
    pub fn references(&self) -> &[Target] {
        &self.references
    }

    fn show_references(&mut self, targets: Vec<Target>, cx: &mut Context<Self>) {
        self.references = targets;
        cx.notify();
    }

    /// Closes the references list.
    pub fn close_references(&mut self, cx: &mut Context<Self>) {
        self.references.clear();
        cx.notify();
    }

    /// Moves the caret to one of the listed uses, or names its file when it is elsewhere.
    pub fn open_reference(&mut self, target: &Target, window: &mut Window, cx: &mut Context<Self>) {
        if target.path.as_deref() != Some(self.path.as_path()) {
            let line = target.range.start.line + 1;
            self.server = format!("used in {} on line {line}", file_name(&target.uri)).into();
            return cx.notify();
        }
        let start = target.range.start;
        self.editor.update(cx, |state, cx| state.set_cursor_position(start, window, cx));
    }

    /// Lists every use of the symbol at the caret, as ⇧F12 does.
    pub fn find_references(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else { return };
        let (doc, position) = self.doc_and_caret(cx);
        let answer = ask(cx, |reply| worker.references(doc, position, reply));
        self.finding = cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| match answer {
                Ok(targets) if targets.is_empty() => {
                    this.server = "no uses found".into();
                    cx.notify();
                }
                Ok(targets) => this.show_references(targets, cx),
                Err(error) => {
                    this.server = format!("{error}").into();
                    cx.notify();
                }
            });
        });
    }

    fn doc_and_caret(&self, cx: &App) -> (Doc, Position) {
        let state = self.editor.read(cx);
        (Doc { path: self.path.clone(), text: state.value().to_string() }, state.cursor_position())
    }

    /// Asks the server what is wrong with the buffer and underlines it. A set lands only if the
    /// buffer still holds the text it was worked out for, so an underline never sits on the wrong
    /// line; a newer check covers the rest.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else { return };
        let (doc, _) = self.doc_and_caret(cx);
        let text = doc.text.clone();
        let answer = ask(cx, |reply| worker.diagnostics(doc, reply));
        self.checking = cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| {
                if this.editor.read(cx).value().as_ref() != text {
                    return;
                }
                match answer {
                    Ok(diagnostics) => {
                        this.problems = summary(&diagnostics);
                        beui::code_editor::set_diagnostics(&this.editor, diagnostics, cx);
                    }
                    Err(LspError::Superseded) => return,
                    Err(error) => this.problems = format!("{error}").into(),
                }
                cx.notify();
            });
        });
    }

    /// Checks the buffer again once typing pauses for [`RECHECK_AFTER`], as Zed does.
    fn schedule_recheck(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            return;
        }
        self.recheck = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RECHECK_AFTER).await;
            _ = this.update(cx, |this, cx| this.check(cx));
        });
    }
}

/// One line for the status row.
fn summary(diagnostics: &[Diagnostic]) -> SharedString {
    let first = diagnostics.first().map(|d| d.message.as_str()).unwrap_or_default();
    match diagnostics.len() {
        0 => "nothing wrong".into(),
        1 => format!("1 problem: {first}").into(),
        n => format!("{n} problems, first: {first}").into(),
    }
}

/// Jumps from the caret to the definition, as F12 does: the editor takes focus, then gpui-base asks
/// the server and moves the caret.
pub fn go_to_definition(editor: &Entity<EditorState>, window: &mut Window, cx: &mut App) {
    editor.update(cx, |state, cx| state.focus(window, cx));
    window.dispatch_action(Box::new(input::GoToDefinition), cx);
}

/// Finds or starts the server for `path` on a thread, so the window opens at once. Anything that
/// stops it, such as a language lathe has no server for or one that is not installed, comes back as
/// the sentence the status line shows.
fn start(path: PathBuf) -> oneshot::Receiver<Result<LspWorker, String>> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || drop(tx.send(workers().for_file(&path).map_err(|e| e.to_string()))));
    rx
}

/// Sends one question to the worker and waits for its answer off the main thread.
fn ask<T: Send + 'static>(cx: &App, send: impl FnOnce(Reply<T>)) -> Task<Result<T, LspError>> {
    let (tx, rx) = oneshot::channel();
    send(Box::new(move |answer| drop(tx.send(answer))));
    cx.background_spawn(async move { rx.await.unwrap_or(Err(LspError::Closed)) })
}

/// The editor's view of the server.
struct EditorLsp {
    worker: LspWorker,
    path: PathBuf,
    last: LastNavigation,
}

impl EditorLsp {
    fn doc(&self, text: &Rope) -> Doc {
        Doc { path: self.path.clone(), text: text.to_string() }
    }
}

impl DefinitionProvider for EditorLsp {
    /// Where a Cmd-click goes. The answer is also kept in `last`, so the click can list several
    /// uses instead of jumping to the first.
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        let (doc, position) = (self.doc(text), text.offset_to_position(offset));
        let answer = ask(cx, |reply| self.worker.navigate(doc, position, reply));
        let last = self.last.clone();
        cx.background_spawn(async move {
            let navigation = answer.await?;
            let links = navigation.targets.iter().map(link).collect();
            if let Ok(mut last) = last.lock() {
                *last = Some(navigation);
            }
            Ok(links)
        })
    }
}

impl HoverProvider for EditorLsp {
    fn hover(&self, text: &Rope, offset: usize, _window: &mut Window, cx: &mut App) -> Task<anyhow::Result<Option<Hover>>> {
        let (doc, position) = (self.doc(text), text.offset_to_position(offset));
        let answer = ask(cx, |reply| self.worker.hover(doc, position, reply));
        cx.background_spawn(async move { Ok(answer.await?) })
    }
}

/// A target as gpui-base's link, which selects the target's name when it jumps there.
fn link(target: &Target) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: target.uri.clone(),
        target_range: target.range,
        target_selection_range: target.range,
    }
}

/// What to do when gpui-base is about to jump: `true` means it was handled here.
type ShowDocument = Rc<dyn Fn(&ShowDocumentParams, &mut App) -> bool>;

/// Gives `state` the server's definitions and hover cards. `show` decides each jump first: it lists
/// several uses, names a file the story cannot show, or lets gpui-base move the caret.
fn attach(state: &Entity<EditorState>, path: &Path, worker: LspWorker, last: LastNavigation, show: ShowDocument, cx: &mut App) {
    let provider = Rc::new(EditorLsp { worker, path: path.to_path_buf(), last });
    state.update(cx, |state, cx| {
        let lsp = state.lsp_mut();
        lsp.definition_provider = Some(provider.clone());
        lsp.hover_provider = Some(provider);
        lsp.show_document = Some(Rc::new(move |params: &ShowDocumentParams, _: &mut Window, cx: &mut App| show(params, cx)));
        state.refresh(cx);
    });
}

/// The file name a server's URI names, decoded, for a status line or a list row.
pub fn file_name(uri: &Uri) -> String {
    match uri_to_path(uri) {
        Some(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        None => uri.as_str().to_string(),
    }
}
