/// How long after the review's last change it is written to the data folder.
pub(super) const SAVE_AFTER: std::time::Duration = std::time::Duration::from_millis(500);

/// How far past the view the list lays out rows.
pub(super) const OVERDRAW: f32 = 160.;

/// How long a row's arrival is kept: well past its entrance.
pub(super) const ARRIVAL_KEPT: std::time::Duration = std::time::Duration::from_secs(2);

/// What the session tells the shell.
pub enum SessionEvent {
    /// Its title, status or id changed: the sidebar and the tabs draw it again.
    Changed,
    /// The reader asked to review a turn (`None` for the whole session) at a file.
    Review { turn: Option<usize>, path: Option<String> },
    /// The reader asked to open a file in the editor.
    OpenFile(String),
    /// The reader picked another agent for this session before its first message, by its backend's name.
    ChooseAgent(String),
    /// The reader pressed a pull request's chip in the agent's text.
    OpenPull(atelier_ui::PrChipData),
    /// The reader named it: the name is kept across launches.
    Renamed,
    /// The reader pressed the session's pull request card.
    ShowPull(atelier_forge::PullRef),
    /// A turn ended, or the history loaded: the agent's text is whole, and its #N can be looked up.
    TextSettled,
    /// Something the tasks linked to this session should hear.
    Task(crate::tasks::signal::TaskEvent),
    /// The reader pressed the task chip in the header.
    OpenTask,
    /// The reader pressed the panel's close button.
    Close,
    /// The reader asked, in the panel's menu, for a new session in this project.
    NewSession,
    /// The reader asked, in the panel's menu, to archive this session.
    Archive,
    /// The reader ran `/files`: the Files view comes to the front.
    ShowFiles,
    /// The reader ran `/tasks`: the project's tasks come to the right pane.
    ShowTasks,
}
