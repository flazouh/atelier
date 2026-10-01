//! The running language servers: one per (server, project root), shared by every file in that
//! project, and started again if it stopped. This is how an editor asks for "the server for this
//! file" without knowing which language the file is in.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use atelier_project::Project;

use crate::{LspError, LspWorker, Store, Unavailable, find_root, language_id, server_for};

/// Why a file has no server, in words the status line can show as they are.
#[derive(Debug)]
pub enum NoServer {
    /// atelier knows no language for the file's extension.
    UnknownLanguage(String),
    /// The language has no server in the registry.
    NoServerFor(&'static str),
    /// The server is known but its program is not installed, and atelier cannot download it.
    NotInstalled { server: &'static str, install: &'static str },
    /// atelier tried to download the server and could not.
    DownloadFailed { server: &'static str, reason: String },
    /// The program ran but the handshake failed.
    Failed { server: &'static str, error: LspError },
}

impl fmt::Display for NoServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(extension) => write!(f, "no language server for .{extension} files"),
            Self::NoServerFor(language) => write!(f, "no language server for {language}"),
            Self::NotInstalled { server, install } => write!(f, "{server} is not installed: {install}"),
            Self::DownloadFailed { server, reason } => write!(f, "could not download {server}: {reason}"),
            Self::Failed { server, error } => write!(f, "{server} did not start: {error}"),
        }
    }
}

/// Every server atelier has started, by (server name, project root).
pub struct Workers {
    /// Where the servers start: the project they serve.
    project: Arc<dyn Project>,
    /// Every server runs at the project's root, instead of the folder of the marker nearest a file,
    /// which only a local disk can be searched for.
    at_project_root: bool,
    running: Mutex<HashMap<(&'static str, PathBuf), LspWorker>>,
    store: Store,
    ready: Duration,
    ask: Duration,
}

impl Workers {
    /// Servers come from `store` and start through `project`. `ready` bounds each server's handshake,
    /// `ask` each request.
    pub fn new(project: Arc<dyn Project>, store: Store, ready: Duration, ask: Duration) -> Self {
        Self { project, at_project_root: false, running: Mutex::default(), store, ready, ask }
    }

    /// Runs every server at the project's root, for a project on another host.
    pub fn at_project_root(mut self) -> Self {
        self.at_project_root = true;
        self
    }

    /// The worker for `path`'s language and project: the one already running there, or a new one
    /// when there is none or its server stopped. It can block for a download and a handshake, so call
    /// it off the UI thread. `report` hears each download start.
    ///
    /// A download runs outside the lock, so it never holds up a server that is already there. The
    /// lock is held while a server starts, so two files of one project never start two.
    pub fn for_file(&self, path: &Path, report: &dyn Fn(String)) -> Result<LspWorker, NoServer> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let language = language_id(path).ok_or(NoServer::UnknownLanguage(extension))?;
        let spec = server_for(language).ok_or(NoServer::NoServerFor(language))?;
        let root = match self.at_project_root {
            true => self.project.root().to_path_buf(),
            false => find_root(&crate::worker::canonical(path), spec.root_markers),
        };
        let key = (spec.name, root);
        if let Some(worker) = self.running(&key) {
            return Ok(worker);
        }
        let launch = self.store.launch(spec, report).map_err(|unavailable| match unavailable {
            Unavailable::NotInstalled => NoServer::NotInstalled { server: spec.name, install: spec.install },
            Unavailable::Failed(reason) => NoServer::DownloadFailed { server: spec.name, reason },
        })?;
        let mut running = self.running.lock().expect("the worker map is not poisoned");
        if let Some(worker) = running.get(&key).filter(|w| w.is_alive()) {
            return Ok(worker.clone());
        }
        let worker = LspWorker::start(&*self.project, spec, &launch, key.1.clone(), self.ready, self.ask)
            .map_err(|error| NoServer::Failed { server: spec.name, error })?;
        running.insert(key, worker.clone());
        Ok(worker)
    }

    /// The live worker for (server, root), if one runs.
    fn running(&self, key: &(&'static str, PathBuf)) -> Option<LspWorker> {
        let running = self.running.lock().expect("the worker map is not poisoned");
        running.get(key).filter(|w| w.is_alive()).cloned()
    }
}

#[cfg(test)]
mod tests;
