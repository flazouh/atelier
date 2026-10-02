//! The "Pull request" and "Pull requests" stories, laid out like GitQuiet's pull request and working set
//! screens (`site/public/store/pull-request.png` and `working-set.png`), in atelier-ui's look.
//!
//! The pull request: a left rail (unsent comments, checks, the conversation, the box for the whole pull
//! request, the verdict, the commits) and a right pane (the seen bar, the changed file tree, and the
//! diff on its card with a thread in place). GitQuiet's keys work while no box has focus: `s` and `w`
//! move between files, `x` marks one seen, `r` is review mode, ⌘B folds the rail and ⌘⇧B the tree.
//!
//! It fits any pane from an 1100px window up (`fit`): the rail and the right pane share the width,
//! the tree folds away first when the diff would get too narrow, then the rail narrows. ⌘⇧B brings the
//! tree back, and the choice holds until the next ⌘⇧B.
//!
//! The diff is read, not typed in, and its new side is the file at the pull request's head, so the
//! language server answers on it as in the Editor story (`editor_lsp`), with rust-analyzer on a small
//! crate written to disk (`pr_fixture`). The removed rows get nothing.
//!
//! - Hover a name for its card; ⌘-click it or press F12 to go to its definition. In this file the caret
//!   moves; in another changed file, that file opens and the tree selects it; in a file the pull request
//!   did not change, the file opens Brought In, out of the seen count. Escape or Previous goes back to
//!   the file before, along a stack.
//! - On a declaration, ⌘-click lists its uses.
//! - `u` Uses, `o` Names in this file, `T` Go to name, `t` Go to file each open a finder over the diff;
//!   their caps are on the file's header.
//! - The server's problems underline the new side, and its state is the line under the diff.

mod helpers;
mod structs;
mod types;

pub use helpers::{element, pull_requests};
#[cfg(test)]
pub use helpers::fit;
pub use structs::PrStory;
#[cfg(test)]
pub use structs::Fit;

#[cfg(test)]
use types::{DIFF_MIN, PADDING, RAIL_GAP, RAIL_MAX, RAIL_MIN, TREE, TREE_GAP};

#[cfg(test)]
mod tests;
