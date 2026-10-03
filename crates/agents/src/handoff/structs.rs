/// Where a handoff comes from, as the brief tells the next agent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Origin {
    /// The agent that did the work, in its own words.
    pub agent: String,
    pub title: String,
    /// Where the whole transcript is kept, on the host the next agent runs on.
    pub transcript: Option<String>,
    /// The worktree as it is now: its branch and changed files.
    pub working_state: Option<String>,
}

/// One exchange: what the user asked, what the agent said, and one line per call it made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Turn {
    /// `None` for work the agent did before any message in the history.
    pub user: Option<String>,
    pub said: String,
    pub tools: Vec<String>,
}
