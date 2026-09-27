//! The running language servers: one per (server, project root), shared by every file in that
//! project, and started again if it stopped. This is how an editor asks for "the server for this
//! file" without knowing which language the file is in.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use crate::{LspError, LspWorker, find_program, find_root, language_id, server_for};

/// Why a file has no server, in words the status line can show as they are.
#[derive(Debug)]
pub enum NoServer {
    /// lathe knows no language for the file's extension.
    UnknownLanguage(String),
    /// The language has no server in the registry.
    NoServerFor(&'static str),
    /// The server is known but its program is not installed.
    NotInstalled { server: &'static str, install: &'static str },
    /// The program ran but the handshake failed.
    Failed { server: &'static str, error: LspError },
}

impl fmt::Display for NoServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(extension) => write!(f, "no language server for .{extension} files"),
            Self::NoServerFor(language) => write!(f, "no language server for {language}"),
            Self::NotInstalled { server, install } => write!(f, "{server} is not installed: {install}"),
            Self::Failed { server, error } => write!(f, "{server} did not start: {error}"),
        }
    }
}

/// Every server lathe has started, by (server name, project root).
pub struct Workers {
    running: Mutex<HashMap<(&'static str, PathBuf), LspWorker>>,
    ready: Duration,
    ask: Duration,
}

impl Workers {
    /// `ready` bounds each server's handshake, `ask` each request.
    pub fn new(ready: Duration, ask: Duration) -> Self {
        Self { running: Mutex::default(), ready, ask }
    }

    /// The worker for `path`'s language and project: the one already running there, or a new one
    /// when there is none or its server stopped. It can block for a handshake, so call it off the UI
    /// thread. The lock is held while a server starts, so two files of one project never start two.
    pub fn for_file(&self, path: &Path) -> Result<LspWorker, NoServer> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let language = language_id(path).ok_or(NoServer::UnknownLanguage(extension))?;
        let spec = server_for(language).ok_or(NoServer::NoServerFor(language))?;
        let root = find_root(&crate::worker::canonical(path), spec.root_markers);
        let mut running = self.running.lock().expect("the worker map is not poisoned");
        let key = (spec.name, root.clone());
        if let Some(worker) = running.get(&key).filter(|w| w.is_alive()) {
            return Ok(worker.clone());
        }
        let program = find_program(spec.program)
            .ok_or(NoServer::NotInstalled { server: spec.name, install: spec.install })?;
        let worker = LspWorker::start(spec, &program, root, self.ready, self.ask)
            .map_err(|error| NoServer::Failed { server: spec.name, error })?;
        running.insert(key, worker.clone());
        Ok(worker)
    }
}

#[cfg(test)]
mod tests;
