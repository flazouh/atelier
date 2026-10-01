use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use atelier_ui::RowMap;
use gpui_kit::{
    App,
    AppContext,
    Context,
    Entity,
    SharedString,
    Subscription,
    Task,
    Window,
    base::input::{DefinitionProvider, HoverProvider, InputEvent, Rope, RopeExt},
    component::input::EditorState,
};
use atelier_lsp::{
    Doc, Found, LspError, LspWorker, Symbol, Target, Workers, canonical, client::uri_to_path,
};
use lsp_types::{Diagnostic, Hover, LocationLink, Position, ShowDocumentParams};

use super::types::{Elsewhere, LastNavigation, RECHECK_AFTER, SECONDARY, ShowDocument, Starting};
use super::helpers::{ask, attach, file_name, link, start, summary};

/// A jump out of this file: the file, and the place in it, in that file's own rows.
#[derive(Clone, Debug)]
pub struct Jump {
    pub path: PathBuf,
    pub position: Position,
}

/// The file's rows as shown: which the server's rows are, and back. A review's hunks move as the reader
/// decides them, so the map can change ([`EditorSession::set_rows`]); every holder shares it.
#[derive(Clone)]
pub(super) struct Rows {
    pub(super) path: PathBuf,
    pub(super) map: Arc<Mutex<RowMap>>,
}

impl Rows {
    pub(super) fn new(path: PathBuf, map: RowMap) -> Self {
        Self { path, map: Arc::new(Mutex::new(map)) }
    }

    pub(super) fn map(&self) -> std::sync::MutexGuard<'_, RowMap> {
        self.map.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(super) fn to_head(&self, position: Position) -> Option<Position> {
        Some(Position { line: self.map().to_head(position.line as usize)? as u32, ..position })
    }

    pub(super) fn to_view(&self, position: Position) -> Position {
        Position { line: self.map().to_view(position.line as usize) as u32, ..position }
    }

    pub(super) fn range_to_view(&self, range: lsp_types::Range) -> lsp_types::Range {
        lsp_types::Range { start: self.to_view(range.start), end: self.to_view(range.end) }
    }

    /// `target` in shown rows when it is in this file.
    pub(super) fn target(&self, target: Target) -> Target {
        if target.path.as_deref() != Some(self.path.as_path()) {
            return target;
        }
        Target { range: self.range_to_view(target.range), ..target }
    }

    pub(super) fn symbol(&self, symbol: Symbol) -> Symbol {
        match uri_to_path(&symbol.uri).map(|p| canonical(&p)) {
            Some(path) if path == self.path => Symbol { range: self.range_to_view(symbol.range), ..symbol },
            _ => symbol,
        }
    }

    pub(super) fn doc(&self, shown: &str) -> Doc {
        Doc { path: self.path.clone(), text: self.map().head_text(shown) }
    }
}

/// One file's language server: it starts the server, hands the editor its definitions and hover
/// cards, keeps the problems current, lists references, and says what it is doing.
pub struct EditorSession {
    pub(super) editor: Entity<EditorState>,
    pub(super) path: PathBuf,
    pub(super) rows: Rows,
    pub(super) elsewhere: Option<Elsewhere>,
    pub(super) worker: Option<LspWorker>,
    /// The server's state, or where the last definition outside this file is.
    pub(super) server: SharedString,
    /// What the last check found.
    pub(super) problems: SharedString,
    /// The uses of a symbol, when there is more than one to choose from.
    pub(super) references: Vec<Target>,
    pub(super) last: LastNavigation,
    pub(super) checking: Task<()>,
    pub(super) recheck: Task<()>,
    pub(super) finding: Task<()>,
    pub(super) _start: Task<()>,
    pub(super) _edits: Subscription,
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

    pub(super) fn started(&mut self, started: Result<LspWorker, String>, cx: &mut Context<Self>) {
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

    pub(super) fn show_references(&mut self, targets: Vec<Target>, cx: &mut Context<Self>) {
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
    pub(super) fn doc_and_caret(&self, cx: &App) -> Option<(Doc, Position)> {
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
                        atelier_ui::code_editor::set_diagnostics(&this.editor, diagnostics, cx);
                    }
                    Err(LspError::Superseded) => return,
                    Err(error) => this.problems = format!("{error}").into(),
                }
                cx.notify();
            });
        });
    }

    /// Checks the buffer again once typing pauses for [`RECHECK_AFTER`], as Zed does.
    pub(super) fn schedule_recheck(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_none() {
            return;
        }
        self.recheck = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RECHECK_AFTER).await;
            _ = this.update(cx, |this, cx| this.check(cx));
        });
    }
}

/// The editor's view of the server.
pub(super) struct EditorLsp {
    pub(super) worker: LspWorker,
    pub(super) rows: Rows,
    pub(super) last: LastNavigation,
}

impl EditorLsp {
    /// The file as the server reads it, and the offset's place in its rows; `None` on a removed row.
    pub(super) fn ask_at(&self, text: &Rope, offset: usize) -> Option<(Doc, Position)> {
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
