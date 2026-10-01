use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc},
};

use futures_channel::{mpsc, oneshot};
use gpui_kit::{
    App,
    AppContext,
    Entity,
    SharedString,
    Task,
    Window,
    base::input::{self},
    component::input::EditorState,
};
use atelier_lsp::{LspError, LspWorker, Reply, Target, Workers, client::uri_to_path};
use lsp_types::{Diagnostic, LocationLink, ShowDocumentParams, Uri};

use super::structs::{EditorLsp, Rows};
use super::types::{LastNavigation, ShowDocument, Starting};

/// One line for the status row.
pub(super) fn summary(diagnostics: &[Diagnostic]) -> SharedString {
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
pub(super) fn start(workers: Arc<Workers>, path: PathBuf, cx: &App) -> mpsc::UnboundedReceiver<Starting> {
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
pub(super) fn ask<T: Send + 'static>(cx: &App, send: impl FnOnce(Reply<T>)) -> Task<Result<T, LspError>> {
    let (tx, rx) = oneshot::channel();
    send(Box::new(move |answer| drop(tx.send(answer))));
    cx.background_spawn(async move { rx.await.unwrap_or(Err(LspError::Closed)) })
}

/// A target as gpui-base's link, which selects the target's name when it jumps there.
pub(super) fn link(target: &Target) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: target.uri.clone(),
        target_range: target.range,
        target_selection_range: target.range,
    }
}

/// Gives `state` the server's definitions and hover cards. `show` decides each jump first: it lists
/// several uses, names a file the story cannot show, or lets gpui-base move the caret.
pub(super) fn attach(state: &Entity<EditorState>, rows: &Rows, worker: LspWorker, last: LastNavigation, show: ShowDocument, cx: &mut App) {
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
