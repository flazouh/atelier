//! Shipping what a review kept: the commit of the accepted hunks and the reader's edits, the push, and
//! the pull request (`plans/m4-app.md`). The review pane's strip drives it; every git call runs off the
//! UI thread, through the project, so it works on a remote project as on a local one.

pub mod commit;
pub mod kept;
