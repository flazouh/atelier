use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use atelier_capabilities::{Actor, Registry, tasks::TasksProvider};
use atelier_gateway::{Gateway, Grant, MailTools, MessagingTools, TasksTools, ToolSet};
use atelier_project::Project;
use atelier_tracker::{LocalTasks, ProjectKey, Tracker};
use gpui_kit::Global;

use super::helpers::{agent_actor, person_actor, run_dir, unique_account};

/// The registry of capabilities and, once a session wants it, the gateway over it. Cheap to clone: the copies share one
/// registry and one gateway.
#[derive(Clone)]
pub(crate) struct CapabilityHub {
    inner: Arc<Inner>,
}

struct Inner {
    registry: Arc<RwLock<Registry>>,
    /// Started by the first grant, not at launch: an app whose agents never use it never opens the port.
    gateway: Mutex<Option<Gateway>>,
    /// The trackers that have a provider already, so a worktree of a project does not register it twice.
    trackers: Mutex<Vec<Arc<dyn Tracker>>>,
    me: Actor,
    /// Where the config files go. `None` when the machine has no data folder.
    run_dir: Option<PathBuf>,
    /// The setting `capabilities.agent_tools`.
    enabled: bool,
}

impl Global for CapabilityHub {}

impl CapabilityHub {
    pub(crate) fn new(me: Actor, run_dir: Option<PathBuf>, enabled: bool) -> Self {
        Self {
            inner: Arc::new(Inner {
                registry: Arc::default(),
                gateway: Mutex::new(None),
                trackers: Mutex::new(Vec::new()),
                me,
                run_dir,
                enabled,
            }),
        }
    }

    /// The hub of the running app: the person is whoever uses this machine, and the files go beside the settings.
    pub(crate) fn for_this_app(enabled: bool) -> Self {
        Self::new(person_actor(), run_dir(), enabled)
    }

    /// The way into the gateway for one new agent session of `project`, or `None` when the session should start
    /// without. Runs on a background thread: it may open the project's tracker and bind a port.
    pub(crate) fn grant(&self, project: &dyn Project) -> Option<Grant> {
        if !self.inner.enabled {
            return None;
        }
        // The agent of a remote project runs on its host, which cannot reach this machine's loopback.
        if project.host().is_some() {
            return None;
        }
        match self.try_grant(project) {
            Ok(grant) => Some(grant),
            Err(error) => {
                eprintln!("agent tools are off for this session: {error}");
                None
            }
        }
    }

    fn try_grant(&self, project: &dyn Project) -> io::Result<Grant> {
        self.register_project(project)?;
        let dir = self
            .inner
            .run_dir
            .as_deref()
            .ok_or_else(|| io::Error::other("this machine has no data folder"))?;
        let mut gateway = self.inner.gateway.lock().unwrap_or_else(|p| p.into_inner());
        if gateway.is_none() {
            let registry = &self.inner.registry;
            let sets: Vec<Arc<dyn ToolSet>> = vec![
                Arc::new(TasksTools::new(registry.clone())),
                Arc::new(MessagingTools::new(registry.clone())),
                Arc::new(MailTools::new(registry.clone())),
            ];
            *gateway = Some(Gateway::start(sets)?);
        }
        gateway
            .as_ref()
            .ok_or_else(|| io::Error::other("the gateway is not running"))?
            .grant(agent_actor(&self.inner.me), dir)
    }

    /// Gives the registry the tasks of `project`, once for each tracker. The account is the folder's name.
    fn register_project(&self, project: &dyn Project) -> io::Result<()> {
        let tracker = project
            .tracker()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut known = self
            .inner
            .trackers
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if known
            .iter()
            .any(|kept| std::ptr::addr_eq(Arc::as_ptr(kept), Arc::as_ptr(&tracker)))
        {
            return Ok(());
        }
        let key = ProjectKey::Local {
            path: project.root().to_string_lossy().into_owned(),
        };
        let mut registry = self
            .inner
            .registry
            .write()
            .unwrap_or_else(|p| p.into_inner());
        let account = unique_account(&registry, key.folder());
        let provider: Arc<dyn TasksProvider> = Arc::new(LocalTasks::new(
            tracker.clone(),
            &account,
            self.inner.me.clone(),
        ));
        registry.add_tasks(provider);
        known.push(tracker);
        Ok(())
    }
}
