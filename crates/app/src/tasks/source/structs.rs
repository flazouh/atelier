use std::sync::Arc;

use atelier_capabilities::{
    Actor, Capabilities, Operation, Ref, Registry,
    tasks::{Category, Label, Project, Status, TasksProvider},
};
use atelier_project::Project as OpenedProject;
use atelier_tracker::{LocalTasks, Tracker};
use atelier_ui::task_model::TaskStatus;
use gpui_kit::SharedString;

use super::super::map;

/// One provider and account, as the switcher lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub provider: String,
    pub account: String,
}

impl Choice {
    /// The provider as a person reads it: `Linear`.
    pub fn name(&self) -> String {
        let mut name = self.provider.chars();
        let head: String = name.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
        format!("{head}{}", name.as_str())
    }

    /// The words of its row: `Linear · acme`.
    pub fn words(&self) -> SharedString {
        format!("{} · {}", self.name(), self.account).into()
    }
}

/// The tasks providers of a project, and which one the screen shows.
pub struct TasksSource {
    registry: Registry,
    selected: Option<Choice>,
    /// The project's own tracker, for the rules and the session and pull request links (local-only for now).
    local: Option<Arc<dyn Tracker>>,
}

impl TasksSource {
    /// The project's local tasks as the `local` provider. Blocks (a remote project asks its host), so it is
    /// never called on the UI thread. `me` is the person using the app.
    pub fn open(project: &Arc<dyn OpenedProject>, me: &str) -> Result<Self, String> {
        let tracker = project.tracker().map_err(|error| error.to_string())?;
        let account = project.root().file_name().map_or_else(|| "project".to_string(), |n| n.to_string_lossy().into_owned());
        let local = LocalTasks::new(tracker.clone(), &account, Actor::person(me, me));
        Ok(Self::from_providers([Arc::new(local) as Arc<dyn TasksProvider>]).with_local(tracker))
    }

    /// A source that holds `providers`, the first of them chosen.
    pub fn from_providers(providers: impl IntoIterator<Item = Arc<dyn TasksProvider>>) -> Self {
        let mut source = Self { registry: Registry::new(), selected: None, local: None };
        providers.into_iter().for_each(|p| source.add(p));
        source
    }

    pub fn with_local(mut self, tracker: Arc<dyn Tracker>) -> Self {
        self.local = Some(tracker);
        self
    }

    /// Adds a provider to the project's registry. The first one added is the one shown.
    pub fn add(&mut self, provider: Arc<dyn TasksProvider>) {
        let choice = Choice { provider: provider.provider().to_string(), account: provider.account().to_string() };
        self.registry.add_tasks(provider);
        self.selected.get_or_insert(choice);
    }

    /// Every provider and account, in the registry's order.
    pub fn choices(&self) -> Vec<Choice> {
        self.registry.all_tasks().iter().map(|p| Choice { provider: p.provider().to_string(), account: p.account().to_string() }).collect()
    }

    pub fn selected(&self) -> Option<usize> {
        let at = self.selected.as_ref()?;
        self.choices().iter().position(|c| c == at)
    }

    /// Shows the provider at `index` of [`choices`](Self::choices). `false` when there is none.
    pub fn select(&mut self, index: usize) -> bool {
        let Some(choice) = self.choices().into_iter().nth(index) else { return false };
        self.selected = Some(choice);
        true
    }

    /// The provider the screen shows.
    pub fn provider(&self) -> Option<Arc<dyn TasksProvider>> {
        let at = self.selected.as_ref()?;
        self.registry.tasks(&at.provider, &at.account)
    }

    pub fn local(&self) -> Option<Arc<dyn Tracker>> {
        self.local.clone()
    }
}

/// What the shown provider offers and names, read when the tasks are: what it can do, its statuses, labels and
/// projects. The screen shows only what this lists, and the map reads the names of references from it.
#[derive(Clone, Debug)]
pub struct Vocabulary {
    provider: String,
    account: String,
    caps: Capabilities,
    statuses: Vec<Status>,
    labels: Vec<Label>,
    projects: Vec<Project>,
}

impl Default for Vocabulary {
    /// Before the first reading: the core calls and the six plain statuses, so no control hides for nothing.
    fn default() -> Self {
        let caps = Capabilities { operations: Capabilities::CORE.to_vec(), ..Capabilities::default() };
        Self::new("local", "project", caps, Vec::new(), Vec::new(), Vec::new())
    }
}

impl Vocabulary {
    /// `statuses` empty means a provider with no custom states: the six plain ones.
    pub fn new(provider: &str, account: &str, caps: Capabilities, statuses: Vec<Status>, labels: Vec<Label>, projects: Vec<Project>) -> Self {
        let statuses = if statuses.is_empty() { Self::plain() } else { statuses };
        Self { provider: provider.into(), account: account.into(), caps, statuses, labels, projects }
    }

    fn plain() -> Vec<Status> {
        [Category::Backlog, Category::Todo, Category::InProgress, Category::InReview, Category::Done, Category::Canceled].into_iter().map(Status::plain).collect()
    }

    pub fn can(&self, operation: Operation) -> bool {
        self.caps.can(operation)
    }

    /// Stops offering a call a provider listed but answered with `Unsupported`.
    pub fn disable(&mut self, operation: Operation) {
        self.caps.operations.retain(|o| *o != operation);
    }

    /// The id the provider gives the status that screen status stands for, when it has one.
    pub fn status_id(&self, status: TaskStatus) -> Option<String> {
        let category = map::status_to(status);
        self.statuses.iter().find(|s| s.category == category).map(|s| s.id.clone())
    }

    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// The reference of the label named `name`. A name the provider does not list takes the form its own
    /// references have, so a provider that makes labels on first use can read it.
    pub fn label_ref(&self, name: &str) -> Ref {
        self.labels.iter().find(|l| l.name == name).map_or_else(|| self.made(name), |l| l.reference.clone())
    }

    pub fn project_ref(&self, name: &str) -> Ref {
        self.projects.iter().find(|p| p.name == name).map_or_else(|| self.made(name), |p| p.reference.clone())
    }

    fn made(&self, id: &str) -> Ref {
        Ref { capability: "tasks".into(), provider: self.provider.clone(), account: self.account.clone(), id: id.into() }
    }

    /// The name a label or a project reference shows as. A reference the lists lack shows its id.
    pub fn name_of(&self, reference: &Ref) -> SharedString {
        let label = self.labels.iter().find(|l| l.reference == *reference).map(|l| &l.name);
        let project = self.projects.iter().find(|p| p.reference == *reference).map(|p| &p.name);
        label.or(project).map_or_else(|| reference.id.clone().into(), |name| name.clone().into())
    }
}
