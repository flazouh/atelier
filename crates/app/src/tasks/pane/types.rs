use atelier_ui::task_list_model::Filters;
use atelier_ui::task_model::{TaskData, TaskStatus};
use gpui_kit::SharedString;
use atelier_capabilities::Ref;

/// Under this width the board would clip, so the pane shows the list and hides the switch.
pub const BOARD_LEAST: f32 = 560.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    Board,
}

pub(super) enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}

/// What went wrong with the provider that the reader can act on or wait out. The tasks already read stay on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// There is no connection: a banner with Retry.
    Offline,
    /// The provider asks for a wait, in milliseconds: a banner with the wait.
    Wait(u64),
    /// The reader is not signed in: an empty state with a button to Settings.
    SignedOut,
}

/// How the screen answers an error of a call. See [`react`](super::helpers::react).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reaction {
    Raise(Problem),
    /// The call is not offered, so its control goes.
    Hide,
    /// One line, in words.
    Line(String),
}

/// What the pane asks of the app.
pub enum TasksEvent {
    /// The reader asked for a session for this task.
    Start(Ref),
    /// The reader asked for the Accounts section of Settings, to sign in to a provider.
    OpenAccounts,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    List,
    Board,
    View,
    None,
}

/// Which of the project's tasks the pane shows, as the Tasks sidebar names them. The filter chips of the
/// list narrow it further.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// Assigned to the reader.
    Mine,
    /// To do, in progress or in review.
    Active,
    Backlog,
    #[default]
    All,
    /// Carrying the label of this name.
    Label(SharedString),
    /// Assigned to the agent of this name.
    Agent(SharedString),
}

impl Scope {
    pub const VIEWS: [Self; 4] = [Self::Mine, Self::Active, Self::Backlog, Self::All];

    /// The scope's name, over the pane.
    pub fn title(&self) -> SharedString {
        match self {
            Self::Mine => "My tasks".into(),
            Self::Active => "Active".into(),
            Self::Backlog => "Backlog".into(),
            Self::All => "All tasks".into(),
            Self::Label(name) | Self::Agent(name) => name.clone(),
        }
    }

    /// `base` with the scope in place of the filters a scope sets, so a text or a priority chosen in the list
    /// stays across scopes.
    pub fn filters(&self, base: &Filters) -> Filters {
        let mut f = Filters { mine: false, assignee: None, label: None, statuses: Vec::new(), ..base.clone() };
        match self {
            Self::Mine => f.mine = true,
            Self::Active => f.statuses = vec![TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::InReview],
            Self::Backlog => f.statuses = vec![TaskStatus::Backlog],
            Self::All => {}
            Self::Label(name) => f.label = Some(name.clone()),
            Self::Agent(name) => f.assignee = Some(name.clone()),
        }
        f
    }

    /// How many of `tasks` the scope holds, before any chip.
    pub fn count(&self, tasks: &[TaskData], me: &str) -> usize {
        let f = self.filters(&Filters::default());
        tasks.iter().filter(|t| f.keeps(t, me)).count()
    }
}
