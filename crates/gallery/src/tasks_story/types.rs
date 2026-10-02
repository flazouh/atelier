#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    List,
    Board,
    Task,
    Create,
}

pub(super) const TABS: [(Tab, &str); 4] = [(Tab::List, "List"), (Tab::Board, "Board"), (Tab::Task, "Task"), (Tab::Create, "Create")];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    List,
    Board,
    View,
    None,
}

/// How many frames the run has. The meter's number.
pub(super) const FRAMES_OF_RUN: usize = crate::load_story::meter::FRAMES;
