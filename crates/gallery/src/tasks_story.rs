//! The "Tasks" story: a list, a board, one task in full, and the create dialog, over 200 fixture tasks. The
//! four share one set of tasks: a change in one shows in the others. `TASKS_VIEW=list|board|task|create`
//! picks the tab to start on (for screenshots). `GALLERY_SCROLL=1` scrolls a list of `TASK_COUNT` tasks
//! (5,000 by default; with `TASKS_VIEW=board`, the board) applies a filter in the middle of the run and takes it off again, and prints the
//! frame numbers; the frames with a filter are counted apart.

mod fixture;
mod structs;
mod types;

pub use structs::TasksStory;
