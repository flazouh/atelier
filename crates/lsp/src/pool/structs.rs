use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use atelier_project::Project;

use crate::{LspWorker, Store, Unavailable, find_root, language_id, server_for};
use super::types::NoServer;

/// Every server atelier has started, by (server name, project root).
pub struct Workers {
    /// Where the servers start: the project they serve.
    pub(super) project: Arc<dyn Project>,
    /// Every server runs at the project's root, instead of the folder of the marker nearest a file,
    /// which only a local disk can be searched for.
    at_project_root: bool,
    pub(super) running: Mutex<HashMap<(&'static str, PathBuf), LspWorker>>,
    pub(super) store: Store,
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
    pub(super) fn running(&self, key: &(&'static str, PathBuf)) -> Option<LspWorker> {
        let running = self.running.lock().expect("the worker map is not poisoned");
        running.get(key).filter(|w| w.is_alive()).cloned()
    }
}
