//! How the pull request pane splits its width. From 700 px up the rail and the diff share it: the rail takes at most
//! two fifths, the tree folds away first when the diff would get too narrow, and ⌘⇧B brings it back (the choice holds
//! until the next ⌘⇧B). Under 700 px one part shows at a time, Details or Files, as the app does in a narrow window.

mod helpers;
mod structs;
mod types;

pub use helpers::fit;
pub use structs::Fit;
pub use types::{
    DIFF_MIN, FOLD_AFTER, NARROW, PADDING, PAGE, Part, RAIL_GAP, RAIL_MAX, RAIL_SHARE, TREE,
    TREE_GAP,
};
