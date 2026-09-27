//! The Editor story's language server, wired into the editor the way Zed does it: hold ⌘ (ctrl on
//! Linux) over a symbol to underline it and click to jump there; rest the pointer on a symbol for its
//! hover card; F12 jumps from the caret; problems are checked on open and again whenever typing
//! pauses. Nothing here blocks the window. Every answer comes from [`LspWorker`], which owns the
//! server on a thread of its own.

use std::{path::PathBuf, rc::Rc, time::Duration};

use futures_channel::oneshot;
use gpui_kit::{
    App, AppContext, Context, Entity, SharedString, Subscription, Task, Window,
    base::input::{self, DefinitionProvider, HoverProvider, InputEvent, Rope, RopeExt},
    component::input::EditorState,
};
use lathe_lsp::{DocumentSync, LspClient, LspError, LspWorker, Reply, definition_links};
use lsp_types::{Diagnostic, Hover, LocationLink, ShowDocumentParams, Uri};

/// How long one question may take. rust-analyzer answers in milliseconds once it has indexed.
const ASK: Duration = Duration::from_secs(20);
/// How long the server may take to start and shake hands.
const READY: Duration = Duration::from_secs(30);
/// How long typing must pause before the buffer is checked again. docs/code-editor.md sets 150ms.
const RECHECK_AFTER: Duration = Duration::from_millis(150);
/// The key a Zed user holds to follow a symbol.
const SECONDARY: &str = if cfg!(target_os = "macos") { "⌘" } else { "ctrl" };

/// One editor's language server: it starts the server, hands the editor its definitions and hover
/// cards, keeps the problems current, and says what it is doing.
pub struct EditorSession {
    editor: Entity<EditorState>,
    worker: Option<LspWorker>,
    /// The server's state, or where the last definition outside this file is.
    server: SharedString,
    /// What the last check found.
    problems: SharedString,
    checking: Task<()>,
    recheck: Task<()>,
    _start: Task<()>,
    _edits: Subscription,
}

impl EditorSession {
    /// Starts rust-analyzer over `text` and attaches it to `editor` once it is ready.
    pub fn new(editor: Entity<EditorState>, text: &str, cx: &mut Context<Self>) -> Self {
        let started = start(text.to_string());
        let _start = cx.spawn(async move |this, cx| {
            let started = started.await.unwrap_or_else(|_| Err("the server thread stopped".into()));
            _ = this.update(cx, |this, cx| this.started(started, cx));
        });
        let _edits = cx.subscribe(&editor, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.schedule_recheck(cx);
            }
        });
        Self {
            editor,
            worker: None,
            server: "rust-analyzer is starting".into(),
            problems: SharedString::default(),
            checking: Task::ready(()),
            recheck: Task::ready(()),
            _start,
            _edits,
        }
    }

    fn started(&mut self, started: Result<Started, String>, cx: &mut Context<Self>) {
        let Started { worker, name } = match started {
            Ok(started) => started,
            Err(error) => {
                self.server = error.into();
                return cx.notify();
            }
        };
        let session = cx.entity().downgrade();
        let elsewhere: Elsewhere = Rc::new(move |params: &ShowDocumentParams, cx: &mut App| {
            let line = params.selection.map(|r| r.start.line + 1).unwrap_or(1);
            let text = format!("defined in {} on line {line}", display_path(&params.uri));
            _ = session.update(cx, |this, cx| {
                this.server = text.into();
                cx.notify();
            });
        });
        attach(&self.editor, worker.clone(), elsewhere, cx);
        self.worker = Some(worker);
        self.server = format!("{name} is ready: hold {SECONDARY} and click a symbol, or press F12").into();
        self.check(cx);
    }

    /// What the status line says: the server's state, then what the last check found.
    pub fn status(&self) -> SharedString {
        match self.problems.is_empty() {
            true => self.server.clone(),
            false => format!("{} · {}", self.server, self.problems).into(),
        }
    }

    /// Asks the server what is wrong with the buffer and underlines it. A set lands only if the
    /// buffer still holds the text it was worked out for, so an underline never sits on the wrong
    /// line; a newer check covers the rest.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else { return };
        let text = self.editor.read(cx).value().to_string();
        let answer = ask(cx, |reply| worker.diagnostics(text.clone(), reply));
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

/// A server that is ready, and the name it gave.
struct Started {
    worker: LspWorker,
    name: String,
}

/// Writes `text` into a one-file crate on disk and starts rust-analyzer over it, on a thread, so the
/// gallery opens at once. The answer is a message rather than a panic: the gallery must open on a box
/// with no server.
fn start(text: String) -> oneshot::Receiver<Result<Started, String>> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || drop(tx.send(start_blocking(&text))));
    rx
}

fn start_blocking(text: &str) -> Result<Started, String> {
    let dir = std::env::temp_dir().join("lathe-gallery-lsp");
    std::fs::create_dir_all(dir.join("src")).map_err(|_| "the fixture directory is not writable".to_string())?;
    let manifest = "[package]\nname = \"lathe-gallery\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    let file = dir.join("src/lib.rs");
    std::fs::write(dir.join("Cargo.toml"), manifest)
        .and_then(|()| std::fs::write(&file, text))
        .map_err(|_| "the fixture could not be written".to_string())?;
    let program = server_program();
    let (mut client, init) = LspClient::spawn(&program, &[], &dir, READY).map_err(|e| e.to_string())?;
    client.did_open(&file, "rust", 1, text).map_err(|e| e.to_string())?;
    let name = init.server_info.map(|i| i.name).unwrap_or_else(|| "the server".into());
    Ok(Started { worker: LspWorker::start(client, file, DocumentSync::opened(text), ASK), name })
}

/// rust-analyzer from `PATH`, or from `~/.cargo/bin` where rustup puts it. An app opened from the
/// Finder or over SSH often has no `~/.cargo/bin` in its `PATH`.
fn server_program() -> String {
    let on_path = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("rust-analyzer").is_file()));
    let home = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo/bin/rust-analyzer"));
    match home {
        Some(path) if !on_path && path.is_file() => path.to_string_lossy().into_owned(),
        _ => "rust-analyzer".into(),
    }
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
}

impl DefinitionProvider for EditorLsp {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        let (text, position) = (text.to_string(), text.offset_to_position(offset));
        let (tx, rx) = oneshot::channel();
        self.worker.definition(text, position, Box::new(move |answer| drop(tx.send(answer.map(definition_links)))));
        cx.background_spawn(async move { Ok(rx.await.unwrap_or(Err(LspError::Closed))?) })
    }
}

impl HoverProvider for EditorLsp {
    fn hover(&self, text: &Rope, offset: usize, _window: &mut Window, cx: &mut App) -> Task<anyhow::Result<Option<Hover>>> {
        let (text, position) = (text.to_string(), text.offset_to_position(offset));
        let answer = ask(cx, |reply| self.worker.hover(text, position, reply));
        cx.background_spawn(async move { Ok(answer.await?) })
    }
}

/// What to do with a definition in another file.
type Elsewhere = Rc<dyn Fn(&ShowDocumentParams, &mut App)>;

/// Gives `state` the server's definitions and hover cards. A definition in this file moves the caret
/// there, as gpui-base does by itself. One in another file goes to `elsewhere`, since the story has
/// only one file to show; the editor never jumps to a line of the wrong file.
fn attach(state: &Entity<EditorState>, worker: LspWorker, elsewhere: Elsewhere, cx: &mut App) {
    let here = lathe_lsp::client::path_to_uri(worker.path()).ok();
    let provider = Rc::new(EditorLsp { worker });
    state.update(cx, |state, cx| {
        let lsp = state.lsp_mut();
        lsp.definition_provider = Some(provider.clone());
        lsp.hover_provider = Some(provider);
        lsp.show_document = Some(Rc::new(move |params: &ShowDocumentParams, _: &mut Window, cx: &mut App| {
            if Some(&params.uri) == here.as_ref() {
                return false;
            }
            elsewhere(params, cx);
            true
        }));
        state.refresh(cx);
    });
}

/// Where a path from a server answer lives, for a status line.
fn display_path(uri: &Uri) -> String {
    let path = uri.path().as_str();
    PathBuf::from(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}
