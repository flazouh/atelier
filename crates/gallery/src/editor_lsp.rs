//! The Editor story's language server, wired into the editor the way Zed does it: hold ⌘ (ctrl on
//! Linux) over a symbol to underline it and click to jump there; rest the pointer on a symbol for its
//! hover card; F12 jumps from the caret. Nothing here blocks the window. Every answer comes from
//! [`LspWorker`], which owns the server on a thread of its own.

use std::{path::PathBuf, rc::Rc, time::Duration};

use futures_channel::oneshot;
use gpui_kit::{
    App, AppContext, Entity, Task, Window,
    base::input::{DefinitionProvider, HoverProvider, Rope, RopeExt},
    component::input::EditorState,
};
use lathe_lsp::{DocumentSync, LspClient, LspError, LspWorker, Reply, definition_links};
use lsp_types::{GotoDefinitionResponse, Hover, LocationLink, ShowDocumentParams, Uri};

/// How long one question may take. rust-analyzer answers in milliseconds once it has indexed.
const ASK: Duration = Duration::from_secs(20);
/// How long the server may take to start and shake hands.
const READY: Duration = Duration::from_secs(30);

/// A server that is ready, and the name it gave.
pub struct Started {
    pub worker: LspWorker,
    pub name: String,
}

/// Writes `text` into a one-file crate on disk and starts rust-analyzer over it, on a thread, so the
/// gallery opens at once. The answer is a message rather than a panic: the gallery must open on a box
/// with no server.
pub fn start(text: String) -> oneshot::Receiver<Result<Started, String>> {
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
pub fn ask<T: Send + 'static>(cx: &App, send: impl FnOnce(Reply<T>)) -> Task<Result<T, LspError>> {
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
        let answer: Task<Result<Option<GotoDefinitionResponse>, LspError>> =
            ask(cx, |reply| self.worker.definition(text, position, reply));
        cx.background_spawn(async move { Ok(definition_links(answer.await?)) })
    }
}

impl HoverProvider for EditorLsp {
    fn hover(&self, text: &Rope, offset: usize, _window: &mut Window, cx: &mut App) -> Task<anyhow::Result<Option<Hover>>> {
        let (text, position) = (text.to_string(), text.offset_to_position(offset));
        let answer: Task<Result<Option<Hover>, LspError>> = ask(cx, |reply| self.worker.hover(text, position, reply));
        cx.background_spawn(async move { Ok(answer.await?) })
    }
}

/// The document `worker` serves, as the URI a server answers with.
pub fn document_uri(worker: &LspWorker) -> Option<Uri> {
    lathe_lsp::client::path_to_uri(worker.path()).ok()
}

/// What to do with a definition in another file.
pub type Elsewhere = Rc<dyn Fn(&ShowDocumentParams, &mut App)>;

/// Gives `state` the server's definitions and hover cards. A definition in this file moves the caret
/// there, as gpui-base does by itself. One in another file goes to `elsewhere`, since the story has
/// only one file to show; the editor never jumps to a line of the wrong file.
pub fn attach(
    state: &Entity<EditorState>,
    worker: LspWorker,
    elsewhere: Elsewhere,
    cx: &mut App,
) {
    let here = document_uri(&worker);
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
pub fn display_path(uri: &Uri) -> String {
    let path = uri.path().as_str();
    PathBuf::from(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}
