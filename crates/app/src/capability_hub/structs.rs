use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use atelier_capabilities::{Actor, Registry, mail::MailProvider, messaging::MessagingProvider, tasks::TasksProvider};
use atelier_gateway::{Gateway, Grant, MailTools, MessagingTools, TasksTools, ToolSet};
use atelier_project::Project;
use atelier_settings::AccountsSaved;
use atelier_tracker::{LocalTasks, ProjectKey, Tracker};
use gpui_kit::Global;

use super::helpers::{agent_actor, person_actor, run_dir, unique_account};
use crate::accounts::{Built, Row, Rows};

/// The registry of capabilities and, once a session wants it, the gateway over it. Cheap to clone: the copies share one
/// registry and one gateway.
///
/// The hub owns every tasks provider the app has, so the Tasks screen and the agent gateway read the same objects: one
/// `local` provider for each project, and one for each account the person connected (see [`crate::accounts`]).
#[derive(Clone)]
pub(crate) struct CapabilityHub {
    inner: Arc<Inner>,
}

struct Inner {
    registry: Arc<RwLock<Registry>>,
    /// Started by the first grant, not at launch: an app whose agents never use it never opens the port.
    gateway: Mutex<Option<Gateway>>,
    /// The projects that have a provider already, so a worktree of a project does not get a second one.
    locals: Mutex<Vec<Local>>,
    /// The accounts the person connected, and how each stands.
    linked: Mutex<Linked>,
    me: Actor,
    /// Where the config files go. `None` when the machine has no data folder.
    run_dir: Option<PathBuf>,
    /// The setting `capabilities.agent_tools`.
    enabled: bool,
}

/// The `local` provider of one project's tracker.
struct Local {
    tracker: Arc<dyn Tracker>,
    provider: Arc<dyn TasksProvider>,
}

#[derive(Default)]
struct Linked {
    providers: Vec<Arc<dyn TasksProvider>>,
    rows: Rows,
    /// Counts the refreshes, so the answer of an old one is dropped.
    turn: u64,
}

impl Global for CapabilityHub {}

impl CapabilityHub {
    pub(crate) fn new(me: Actor, run_dir: Option<PathBuf>, enabled: bool) -> Self {
        Self {
            inner: Arc::new(Inner {
                registry: Arc::default(),
                gateway: Mutex::new(None),
                locals: Mutex::new(Vec::new()),
                linked: Mutex::new(Linked::default()),
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

    /// Adds a messaging account to the registry that the agent gateway and the Messages screen both read, so what the
    /// agent can reach is what the person sees. A second one for the same provider and account replaces the first.
    #[cfg_attr(
        not(debug_assertions),
        allow(
            dead_code,
            reason = "the Accounts section of Settings registers the real ones; until it lands only the debug demo does"
        )
    )]
    pub(crate) fn add_messaging(&self, provider: Arc<dyn MessagingProvider>) {
        self.inner
            .registry
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .add_messaging(provider);
    }

    /// Every messaging account the app has, in the registry's order. The screen asks again each time it opens, so an
    /// account added later shows.
    pub(crate) fn messaging_providers(&self) -> Vec<Arc<dyn MessagingProvider>> {
        self.inner
            .registry
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .all_messaging()
    }

    /// Adds a mail account to the registry that the agent gateway and the Mail screen both read, so what the agent can reach is
    /// what the person sees. A second one for the same provider and account replaces the first.
    #[cfg_attr(
        not(debug_assertions),
        allow(
            dead_code,
            reason = "the Accounts section of Settings registers the real ones; until it lands only the debug demo does"
        )
    )]
    pub(crate) fn add_mail(&self, provider: Arc<dyn MailProvider>) {
        self.inner
            .registry
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .add_mail(provider);
    }

    /// Every mail account the app has, in the registry's order. The screen asks again each time it opens, so an account added
    /// later shows.
    pub(crate) fn mail_providers(&self) -> Vec<Arc<dyn MailProvider>> {
        self.inner
            .registry
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .all_mail()
    }

    /// The person the app acts for: every change they make in a screen is theirs.
    pub(crate) fn person(&self) -> Actor {
        self.inner.me.clone()
    }

    /// A hub with no gateway, for a screen that runs without the app's own (a test, the gallery).
    pub(crate) fn detached() -> Self {
        Self::new(person_actor(), None, false)
    }

    /// The registry the gateway reads.
    pub(crate) fn registry(&self) -> Arc<RwLock<Registry>> {
        self.inner.registry.clone()
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
        self.local_tasks(project)?;
        let dir = self
            .inner
            .run_dir
            .as_deref()
            .ok_or_else(|| io::Error::other("this machine has no data folder"))?;
        let mut gateway = self.inner.gateway.lock().unwrap_or_else(|p| p.into_inner());
        if gateway.is_none() {
            let registry = self.registry();
            let sets: Vec<Arc<dyn ToolSet>> = vec![
                Arc::new(TasksTools::new(registry.clone())),
                Arc::new(MessagingTools::new(registry.clone())),
                Arc::new(MailTools::new(registry)),
            ];
            *gateway = Some(Gateway::start(sets)?);
        }
        gateway
            .as_ref()
            .ok_or_else(|| io::Error::other("the gateway is not running"))?
            .grant(agent_actor(&self.inner.me), dir)
    }

    /// The `local` provider of `project`: one for each tracker, whoever asks, so the Tasks screen and the gateway share
    /// it. The account is the folder's name. The gateway gets it only for a project on this machine, as its agents
    /// cannot reach a remote host's. Blocks (a remote project asks its host): never on the UI thread.
    pub(crate) fn local_tasks(&self, project: &dyn Project) -> io::Result<Arc<dyn TasksProvider>> {
        let tracker = project
            .tracker()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut known = self.inner.locals.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(kept) = known
            .iter()
            .find(|kept| std::ptr::addr_eq(Arc::as_ptr(&kept.tracker), Arc::as_ptr(&tracker)))
        {
            return Ok(kept.provider.clone());
        }
        let key = ProjectKey::Local {
            path: project.root().to_string_lossy().into_owned(),
        };
        let taken: Vec<String> = known
            .iter()
            .map(|k| k.provider.account().to_string())
            .collect();
        let account = unique_account(&taken, key.folder());
        let provider: Arc<dyn TasksProvider> = Arc::new(LocalTasks::new(
            tracker.clone(),
            &account,
            self.inner.me.clone(),
        ));
        if project.host().is_none() {
            self.inner
                .registry
                .write()
                .unwrap_or_else(|p| p.into_inner())
                .add_tasks(provider.clone());
        }
        known.push(Local {
            tracker,
            provider: provider.clone(),
        });
        Ok(provider)
    }

    /// The providers of the accounts the person connected and that work now.
    pub(crate) fn accounts(&self) -> Vec<Arc<dyn TasksProvider>> {
        self.linked().providers.clone()
    }

    /// How each kind of account stands.
    pub(crate) fn rows(&self) -> Rows {
        self.linked().rows.clone()
    }

    /// A refresh begins for `saved`: what is saved reads "Checking" until it ends, and the providers in place stay. Gives
    /// the number of this refresh, which [`install`](Self::install) wants back.
    pub(crate) fn checking(&self, saved: &AccountsSaved) -> u64 {
        let mut linked = self.linked();
        linked.turn += 1;
        let state = |on: bool| if on { Row::Checking } else { Row::Off };
        linked.rows = Rows {
            linear: state(saved.linear.is_some()),
            github: state(saved.github_issues.is_some()),
        };
        linked.turn
    }

    /// Swaps the connected accounts for `built`, in the gateway's registry too. `false`, and nothing changes, when a
    /// newer refresh began since `turn`.
    pub(crate) fn install(&self, built: Built, turn: u64) -> bool {
        let mut linked = self.linked();
        if linked.turn != turn {
            return false;
        }
        let mut registry = self
            .inner
            .registry
            .write()
            .unwrap_or_else(|p| p.into_inner());
        for old in &linked.providers {
            registry.remove_tasks(old.provider(), old.account());
        }
        for new in &built.providers {
            registry.add_tasks(new.clone());
        }
        linked.providers = built.providers;
        linked.rows = built.rows;
        true
    }

    fn linked(&self) -> std::sync::MutexGuard<'_, Linked> {
        self.inner.linked.lock().unwrap_or_else(|p| p.into_inner())
    }
}
