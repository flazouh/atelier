use super::structs::{Activity, Comment, Envelope, Event, Label, NewTask, Page, Patch, Project, Query, Status, Task};
use crate::{Actor, CapError, CapResult, Capabilities, Operation, Ref, Subscription};

/// A provider of tasks: Atelier's own, Linear, GitHub Issues. The one tasks screen and the agent tools talk to this and
/// to nothing else. Every call blocks and may be slow, so none is made on the UI thread.
///
/// A provider lists what it can do in [`capabilities`](Self::capabilities). A call it does not list returns
/// [`CapError::Unsupported`]; the optional calls below do that by default.
pub trait TasksProvider: Send + Sync {
    /// The provider's name in references: `local`, `linear`, `github`.
    fn provider(&self) -> &str;

    /// The account in references: the workspace, the organisation, the project folder.
    fn account(&self) -> &str;

    fn capabilities(&self) -> Capabilities;

    /// The actor the credentials belong to.
    fn whoami(&self) -> CapResult<Actor>;

    /// The tasks that match, most recently changed first unless the query sorts otherwise.
    fn list(&self, query: &Query) -> CapResult<Page<Task>>;

    fn get(&self, task: &Ref) -> CapResult<Task>;

    fn create(&self, new: &NewTask, by: &Actor) -> CapResult<Task>;

    /// Changes the fields in `patch`. `version` is the one the caller read; a task that changed since fails with
    /// [`CapError::Conflict`], which carries the task as it is now.
    fn update(&self, task: &Ref, patch: &Patch, version: &str, by: &Actor) -> CapResult<Task>;

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<Comment>;

    /// The activity of a task, oldest first.
    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<Activity>>;

    fn labels(&self) -> CapResult<Vec<Label>>;

    fn projects(&self) -> CapResult<Vec<Project>>;

    /// Changes from now on. Each call gives its own receiver; drop it to stop.
    fn subscribe(&self) -> CapResult<Subscription<Event>>;

    /// The statuses the provider has, when it has custom states. Without them the six plain ones.
    fn statuses(&self) -> CapResult<Vec<Status>> {
        Err(CapError::unsupported("list statuses"))
    }

    fn delete(&self, _task: &Ref, _by: &Actor) -> CapResult<()> {
        Err(CapError::unsupported("delete a task"))
    }

    /// Every entity, in order, with a cursor to go on. The path to move a team out of a provider.
    fn export(&self, _cursor: Option<&str>) -> CapResult<Page<Envelope>> {
        Err(CapError::unsupported("export"))
    }

    /// Writes a batch in one step. Idempotent: a batch that was written before changes nothing.
    fn import(&self, _batch: &[Envelope]) -> CapResult<usize> {
        Err(CapError::unsupported("import"))
    }

    /// Whether the provider lists `operation`.
    fn can(&self, operation: Operation) -> bool {
        self.capabilities().can(operation)
    }
}
