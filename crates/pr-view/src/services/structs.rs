use std::{path::PathBuf, sync::Arc, time::Duration};

use atelier_forge::Forge;
use atelier_lsp::Workers;
use atelier_project::Project;

use crate::{
    git::PrGit,
    snapshot::{ListSnapshot, Snapshots},
    state::Reviewed,
};

/// What the app tells the view, all as data.
#[derive(Clone)]
pub struct PrConfig {
    /// The reader's login, for "yours" and for the verdict box.
    pub me: String,
    /// Every write button says so and does nothing. Set until a scratch repository is approved.
    pub read_only: bool,
    /// The folder on the project's host for the cache repositories and the head checkouts. Absolute, or
    /// starting with `~/`. Empty (the default): the `pr-view` folder of the project's data folder, where the
    /// project has one, and `~/.local/share/atelier/pr` where it has not (an older atelier kept it there, and what
    /// is there is moved on first use).
    pub remote_data: String,
    /// The folder on this machine for the reviewed-state database and the snapshots.
    pub local_data: PathBuf,
    /// The language servers, the pool the editor uses. `None`: no language features in the diff.
    pub workers: Option<Arc<Workers>>,
    /// How often an open pull request is asked about while things happen.
    pub refresh: Duration,
    /// How often the list is refreshed.
    pub list_refresh: Duration,
    /// The one repository the list holds, for a project's own pane; `None` for the reader's whole
    /// working set, on its Courts.
    pub repo: Option<atelier_forge::RepoRef>,
    /// Where to fetch pull requests from, when it is not the project's own remote for the repository.
    /// For a mirror, or a test.
    pub fetch_url: Option<String>,
}

impl PrConfig {
    pub fn new(me: impl Into<String>, local_data: impl Into<PathBuf>) -> Self {
        Self {
            me: me.into(),
            read_only: false,
            remote_data: String::new(),
            local_data: local_data.into(),
            workers: None,
            refresh: Duration::from_secs(30),
            list_refresh: Duration::from_secs(60),
            fetch_url: None,
            repo: None,
        }
    }

    /// Only `repo`'s pull requests in the list.
    pub fn repo(mut self, repo: atelier_forge::RepoRef) -> Self {
        self.repo = Some(repo);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn workers(mut self, workers: Arc<Workers>) -> Self {
        self.workers = Some(workers);
        self
    }

    pub fn fetch_url(mut self, url: impl Into<String>) -> Self {
        self.fetch_url = Some(url.into());
        self
    }

    pub fn remote_data(mut self, folder: impl Into<String>) -> Self {
        self.remote_data = folder.into();
        self
    }

    pub fn refresh(mut self, refresh: Duration, list: Duration) -> Self {
        self.refresh = refresh;
        self.list_refresh = list;
        self
    }
}

pub struct Services {
    pub forge: Arc<dyn Forge>,
    pub git: Arc<PrGit>,
    pub snapshots: Snapshots,
    pub list_snapshot: ListSnapshot,
    pub reviewed: Arc<Reviewed>,
    pub config: PrConfig,
}

impl Services {
    /// Opens the reader's database and the caches. Small local files, opened at once.
    pub fn open(project: Arc<dyn Project>, forge: Arc<dyn Forge>, config: PrConfig) -> Result<Arc<Self>, String> {
        let dir = config.local_data.join("pr-view");
        let reviewed = Reviewed::open(&dir.join("reviewed.sqlite")).map_err(|e| format!("could not open the reviewed-state database: {e}"))?;
        Ok(Arc::new(Self {
            forge,
            git: Arc::new(match &config.fetch_url {
                Some(url) => PrGit::new(project, &config.remote_data).with_remote(url.clone()),
                None => PrGit::new(project, &config.remote_data),
            }),
            snapshots: Snapshots::new(dir.join("snapshots")),
            list_snapshot: ListSnapshot::scoped(&dir, config.repo.as_ref()),
            reviewed: Arc::new(reviewed),
            config,
        }))
    }
}
