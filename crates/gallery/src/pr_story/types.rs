/// The head row of `src/request.rs` a bot's thread hangs under.
pub(super) const THREAD_ROW: usize = 16;

/// The line under the diff that says what the server is doing.
pub(super) const STATUS_HEIGHT: f32 = 22.;

pub(super) const COMMITS: [&str; 6] =
    ["Detach the byte stream before a second write", "Test an abort between chunks", "Hold the sink until flush", "Name the fields", "Drop the extra render", "Keep the status flag"];

/// The pull request's description, which fills the squash commit's message.
pub(super) const PR_BODY: &str = "A client that aborted between two chunks left the relay writing into a closed sink. The stream now detaches on abort, and a second write does nothing.";

/// The rail's widest and narrowest, the tree's width, and the least the diff card may have.
pub(super) const RAIL_MAX: f32 = 380.;

pub(super) const RAIL_MIN: f32 = 300.;

pub(super) const TREE: f32 = 240.;

pub(super) const DIFF_MIN: f32 = 460.;

/// The pane's padding on both sides, the gap after the rail, and the gap after the tree.
pub(super) const PADDING: f32 = 16.;

pub(super) const RAIL_GAP: f32 = 12.;

pub(super) const TREE_GAP: f32 = 8.;
