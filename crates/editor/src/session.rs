//! A file's language server, wired into the editor the way Zed does it: hold ⌘ (ctrl on Linux) over
//! a symbol to underline it and click to go to its definition, or to its uses when it is the
//! definition; F12 does the same from the caret and ⇧F12 lists the uses; rest the pointer on a symbol
//! for its hover card; problems are checked on open and again whenever typing pauses.
//!
//! Nothing here names a language: the file's path picks the server from `atelier_lsp::servers`, and
//! [`LspWorker`] runs it on a thread of its own, so no answer ever holds the window.
//!
//! A pull request's diff shows removed rows the file does not have. Its session holds a [`RowMap`]:
//! the server reads the file without them, and every row in this file is mapped across at this
//! boundary, so everything else here counts shown rows. A jump into another file goes to the owner
//! ([`Elsewhere`]) when there is one; otherwise the status line names the place.

use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use beui::RowMap;
use gpui_kit::{
    App, AppContext, Context, Entity, SharedString, Subscription, Task, Window,
    base::input::{self, DefinitionProvider, HoverProvider, InputEvent, Rope, RopeExt},
    component::input::EditorState,
};
use atelier_lsp::{Doc, Found, LspError, LspWorker, Navigation, Reply, Symbol, Target, Workers, canonical, client::uri_to_path};
use lsp_types::{Diagnostic, Hover, LocationLink, Position, ShowDocumentParams, Uri};

/// How long one question may take. Servers answer in milliseconds once they have indexed.
pub const ASK: Duration = Duration::from_secs(20);
/// How long a server may take to start and shake hands.
pub const READY: Duration = Duration::from_secs(60);
/// How long typing must pause before the buffer is checked again. docs/code-editor.md sets 150ms.
const RECHECK_AFTER: Duration = Duration::from_millis(150);
/// The key a Zed user holds to follow a symbol.
const SECONDARY: &str = if cfg!(target_os = "macos") { "⌘" } else { "ctrl" };

/// The newest answer to a Cmd-hover, Cmd-click or F12, kept so a click can tell one place to jump to
/// from several to list. The provider writes it off the main thread.
type LastNavigation = Arc<Mutex<Option<Navigation>>>;

/// A jump out of this file: the file, and the place in it, in that file's own rows.
#[derive(Clone, Debug)]
pub struct Jump {
    pub path: PathBuf,
    pub position: Position,
}

/// What the owner does with a jump out of the file.
pub type Elsewhere = Rc<dyn Fn(Jump, &mut Window, &mut App)>;

/// The file's rows as shown: which the server's rows are, and back. A review's hunks move as the reader
/// decides them, so the map can change ([`EditorSession::set_rows`]); every holder shares it.
#[derive(Clone)]
struct Rows {
    path: PathBuf,
    map: Arc<Mutex<RowMap>>,
}

impl Rows {
    fn new(path: PathBuf, map: RowMap) -> Self {
        Self { path, map: Arc::new(Mutex::new(map)) }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, RowMap> {
        self.map.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn to_head(&self, position: Position) -> Option<Position> {
        Some(Position { line: self.map().to_head(position.line as usize)? as u32, ..position })
    }

    fn to_view(&self, position: Position) -> Position {
        Position { line: self.map().to_view(position.line as usize) as u32, ..position }
    }

    fn range_to_view(&self, range: lsp_types::Range) -> lsp_types::Range {
        lsp_types::Range { start: self.to_view(range.start), end: self.to_view(range.end) }
    }

    /// `target` in shown rows when it is in this file.
    fn target(&self, target: Target) -> Target {
        if target.path.as_deref() != Some(self.path.as_path()) {
            return target;
        }
        Target { range: self.range_to_view(target.range), ..target }
    }

    fn symbol(&self, symbol: Symbol) -> Symbol {
        match uri_to_path(&symbol.uri).map(|p| canonical(&p)) {
            Some(path) if path == self.path => Symbol { range: self.range_to_view(symbol.range), ..symbol },
            _ => symbol,
        }
    }

    fn doc(&self, shown: &str) -> Doc {
        Doc { path: self.path.clone(), text: self.map().head_text(shown) }
    }
}

/// What the thread that starts a server says, in order: any downloads, then the result.
enum Starting {
    Downloading(String),
    Done(Result<LspWorker, String>),
}

/// One file's language server: it starts the server, hands the editor its definitions and hover
/// cards, keeps the problems current, lists references, and says what it is doing.
pub struct EditorSession {
    editor: Entity<EditorState>,
    path: PathBuf,
    rows: Rows,
    elsewhere: Option<Elsewhere>,
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
    /// Starts the server for `path`, whose text `editor` holds, from the project's `workers`, and
    /// attaches it once it is ready.
    pub fn new(workers: Arc<Workers>, editor: Entity<EditorState>, path: PathBuf, cx: &mut Context<Self>) -> Self {
        Self::for_review(workers, editor, path, RowMap::default(), None, cx)
    }

    /// The same for a buffer that shows `rows` over the file, as a pull request's diff does, with
    /// jumps into other files handed to `elsewhere`.
    pub fn for_review(
        workers: Arc<Workers>,
        editor: Entity<EditorState>,
        path: PathBuf,
        rows: RowMap,
        elsewhere: Option<Elsewhere>,
        cx: &mut Context<Self>,
    ) -> Self {
        let path = canonical(&path);
        let rows = Rows::new(path.clone(), rows);
        let mut starting = start(workers, path.clone(), cx);
        let _start = cx.spawn(async move |this, cx| {
            loop {
                let started = match starting.next().await {
                    Some(Starting::Downloading(line)) => {
                        _ = this.update(cx, |this, cx| {
                            this.server = format!("{line}, once; later starts use this copy").into();
                            cx.notify();
                        });
                        continue;
                    }
                    Some(Starting::Done(started)) => started,
                    None => Err("the server thread stopped".into()),
                };
                _ = this.update(cx, |this, cx| this.started(started, cx));
                return;
            }
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
            rows,
            elsewhere,
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
        let elsewhere = self.elsewhere.clone();
        let show: ShowDocument = Rc::new(move |params: &ShowDocumentParams, window: &mut Window, cx: &mut App| {
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
            if let (Some(elsewhere), Some(path)) = (&elsewhere, clicked.clone()) {
                elsewhere(Jump { path, position: start.unwrap_or_default() }, window, cx);
                return true;
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
        attach(&self.editor, &self.rows, worker.clone(), self.last.clone(), show, cx);
        self.server = format!("{} is ready: hold {SECONDARY} and click a symbol, or press F12", worker.name()).into();
        self.worker = Some(worker);
        self.check(cx);
    }

    /// The shown rows the file does not have changed: a review's hunk was decided, or its text edited.
    /// The server reads the file with the new map from the next question on, and checks it again. An
    /// answer asked on the old map and shown on the new one can sit a row off until that check lands.
    pub fn set_rows(&mut self, rows: RowMap, cx: &mut Context<Self>) {
        *self.rows.map() = rows;
        self.schedule_recheck(cx);
    }

    /// What the status line says: the server's state, then what the last check found. The segments
    /// part by space when shown, never by a glyph.
    pub fn status(&self) -> Vec<SharedString> {
        match self.problems.is_empty() {
            true => vec![self.server.clone()],
            false => vec![self.server.clone(), self.problems.clone()],
        }
    }

    /// Puts `text` in the status line where the server's state goes.
    pub fn say(&mut self, text: String, cx: &mut Context<Self>) {
        self.server = text.into();
        cx.notify();
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

    /// Moves the caret to one of the listed uses, or hands it to the owner, or names its file, when it
    /// is elsewhere.
    pub fn open_reference(&mut self, target: &Target, window: &mut Window, cx: &mut Context<Self>) {
        if target.path.as_deref() != Some(self.path.as_path()) {
            if let (Some(elsewhere), Some(path)) = (self.elsewhere.clone(), target.path.clone()) {
                return elsewhere(Jump { path, position: target.range.start }, window, cx);
            }
            let line = target.range.start.line + 1;
            self.server = format!("used in {} on line {line}", file_name(&target.uri)).into();
            return cx.notify();
        }
        let start = target.range.start;
        self.editor.update(cx, |state, cx| state.set_cursor_position(start, window, cx));
    }

    /// The uses of the name at `at`, a shown offset such as the one under the pointer, or at the
    /// caret without one; this file's in shown rows. `None` before the server is ready, or on a
    /// removed row, which the file does not have.
    pub fn uses(&self, at: Option<usize>, cx: &App) -> Option<Task<Result<Vec<Target>, LspError>>> {
        let worker = self.worker.clone()?;
        let (doc, caret) = self.doc_and_caret(cx)?;
        let position = match at {
            Some(offset) => self.rows.to_head(self.editor.read(cx).text().offset_to_position(offset))?,
            None => caret,
        };
        let answer = ask(cx, |reply| worker.references(doc, position, reply));
        let rows = self.rows.clone();
        Some(cx.background_spawn(async move { Ok(answer.await?.into_iter().map(|t| rows.target(t)).collect()) }))
    }

    /// The names this file writes down, in text order and shown rows.
    pub fn names(&self, cx: &App) -> Option<Task<Result<Vec<Symbol>, LspError>>> {
        let worker = self.worker.clone()?;
        let doc = self.rows.doc(&self.editor.read(cx).value());
        let answer = ask(cx, |reply| worker.symbols(doc, reply));
        let rows = self.rows.clone();
        Some(cx.background_spawn(async move { Ok(answer.await?.into_iter().map(|s| rows.symbol(s)).collect()) }))
    }

    /// The names anywhere in the project that match `query`.
    pub fn project_names(&self, query: &str, cx: &App) -> Option<Task<Result<Vec<Symbol>, LspError>>> {
        let worker = self.worker.clone()?;
        let doc = self.rows.doc(&self.editor.read(cx).value());
        let answer = ask(cx, |reply| worker.project_symbols(doc, query.to_string(), reply));
        let rows = self.rows.clone();
        Some(cx.background_spawn(async move { Ok(answer.await?.into_iter().map(|s| rows.symbol(s)).collect()) }))
    }

    /// Lists every use of the symbol at the caret, as ⇧F12 does.
    pub fn find_references(&mut self, cx: &mut Context<Self>) {
        let Some(answer) = self.uses(None, cx) else { return };
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

    /// The file as the server reads it, and the caret in its rows; `None` on a removed row.
    fn doc_and_caret(&self, cx: &App) -> Option<(Doc, Position)> {
        let state = self.editor.read(cx);
        Some((self.rows.doc(&state.value()), self.rows.to_head(state.cursor_position())?))
    }

    /// Asks the server what is wrong with the buffer and underlines it. A set lands only if the
    /// buffer still holds the text it was worked out for, so an underline never sits on the wrong
    /// line; a newer check covers the rest.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else { return };
        let shown = self.editor.read(cx).value().to_string();
        let doc = self.rows.doc(&shown);
        let answer = ask(cx, |reply| worker.diagnostics(doc, reply));
        let rows = self.rows.clone();
        self.checking = cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| {
                if this.editor.read(cx).value().as_ref() != shown {
                    return;
                }
                match answer {
                    Ok(diagnostics) => {
                        let diagnostics: Vec<Diagnostic> =
                            diagnostics.into_iter().map(|d| Diagnostic { range: rows.range_to_view(d.range), ..d }).collect();
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
        0 => "No problems".into(),
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

/// Finds or starts the server for `path` on the background executor, so the window opens at once.
/// Anything that stops it, such as a language atelier has no server for or one that is not installed,
/// comes back as the sentence the status line shows. The executor, not a thread of its own, runs it,
/// so a test's scheduler sees every wake.
fn start(workers: Arc<Workers>, path: PathBuf, cx: &App) -> mpsc::UnboundedReceiver<Starting> {
    let (tx, rx) = mpsc::unbounded();
    cx.background_spawn(async move {
        let report = |line| drop(tx.unbounded_send(Starting::Downloading(line)));
        let started = workers.for_file(&path, &report).map_err(|e| e.to_string());
        drop(tx.unbounded_send(Starting::Done(started)));
    })
    .detach();
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
    rows: Rows,
    last: LastNavigation,
}

impl EditorLsp {
    /// The file as the server reads it, and the offset's place in its rows; `None` on a removed row.
    fn ask_at(&self, text: &Rope, offset: usize) -> Option<(Doc, Position)> {
        Some((self.rows.doc(&text.to_string()), self.rows.to_head(text.offset_to_position(offset))?))
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
        let Some((doc, position)) = self.ask_at(text, offset) else { return Task::ready(Ok(Vec::new())) };
        let answer = ask(cx, |reply| self.worker.navigate(doc, position, reply));
        let (last, rows) = (self.last.clone(), self.rows.clone());
        cx.background_spawn(async move {
            let mut navigation = answer.await?;
            navigation.targets = navigation.targets.into_iter().map(|t| rows.target(t)).collect();
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
        let Some((doc, position)) = self.ask_at(text, offset) else { return Task::ready(Ok(None)) };
        let answer = ask(cx, |reply| self.worker.hover(doc, position, reply));
        let rows = self.rows.clone();
        cx.background_spawn(async move {
            Ok(answer.await?.map(|hover| Hover { range: hover.range.map(|r| rows.range_to_view(r)), ..hover }))
        })
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
type ShowDocument = Rc<dyn Fn(&ShowDocumentParams, &mut Window, &mut App) -> bool>;

/// Gives `state` the server's definitions and hover cards. `show` decides each jump first: it lists
/// several uses, names a file the story cannot show, or lets gpui-base move the caret.
fn attach(state: &Entity<EditorState>, rows: &Rows, worker: LspWorker, last: LastNavigation, show: ShowDocument, cx: &mut App) {
    let provider = Rc::new(EditorLsp { worker, rows: rows.clone(), last });
    state.update(cx, |state, cx| {
        let lsp = state.lsp_mut();
        lsp.definition_provider = Some(provider.clone());
        lsp.hover_provider = Some(provider);
        lsp.show_document = Some(Rc::new(move |params: &ShowDocumentParams, window: &mut Window, cx: &mut App| show(params, window, cx)));
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

#[cfg(test)]
mod tests;
