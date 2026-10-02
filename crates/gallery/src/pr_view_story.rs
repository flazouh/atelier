//! The "Pull request view" story: the real view (`atelier-pr-view`) on a real git repository and an
//! in-memory forge. The repository is made on disk when the story opens: a small crate "relay" on main, and
//! pull request 3344 with six commits, fetched into a cache and read by git the way a project's would be.
//! The forge holds its threads, remarks and checks (one failing job with its log), the reader's last
//! review, and a working set for the list. Writes go to the in-memory forge and nowhere else.
//!
//! `PRV_VIEW=list` opens the list first; the default opens the pull request. `PRV_LSP=1` starts the language
//! servers on the head's checkout (rust-analyzer, downloaded once unless `ATELIER_OFFLINE` is set).
//! `PRV_BIG=1` opens a large pull request (`PRV_FILES`, `PRV_COMMENTS`); `GALLERY_SCROLL=1` measures frames on it.
//! `PRV_REAL=owner/name#number` reads a real pull request through `gh`, read only (`PRV_ME` names the reader).

mod structs;
mod types;

pub use structs::PrViewStory;
