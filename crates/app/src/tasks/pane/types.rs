use gpui_kit::SharedString;
use atelier_tracker::TaskId;

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

/// What the pane asks of the app.
pub enum TasksEvent {
    /// The reader asked for a session for this task.
    Start(TaskId),
    /// The tasks were read: this many are open (not Done, not Canceled).
    Counted(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    List,
    Board,
    View,
    None,
}
