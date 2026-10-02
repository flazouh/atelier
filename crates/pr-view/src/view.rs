//! One pull request on the screen, on real data. The rail holds what is said and decided about the whole
//! pull request (checks, conversation, the verdict, the merge, the commits); the right pane holds the files
//! (the seen bar, the tree, and the diff with the threads in place). [`PrModel`](crate::model::PrModel) decides; this reads and
//! writes through the forge and git on background tasks and draws.
//!
//! Every blocking call is made in `background_spawn`. Answers come back as `Msg` on a channel and are
//! taken on the UI thread, one at a time, so a slow forge never stops a frame.

mod helpers;
mod structs;
mod types;

pub use structs::PullView;
pub use types::PullEvent;
