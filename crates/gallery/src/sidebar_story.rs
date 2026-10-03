//! The "Sidebar" story: three projects (one on this machine, one over SSH and connected, one over SSH and
//! reconnecting), and sessions in every status. A session's ⋯ menu offers Handoff: the agents, and for Claude Code its
//! accounts; a click shows as an event on the right. "Live" cycles the statuses so a row can be watched moving,
//! fading its mark and entering. `GALLERY_SCROLL=1` scrolls a sidebar of `SIDEBAR_PROJECTS` projects
//! (50 by default) of `SIDEBAR_SESSIONS` sessions each (40) and prints the frame numbers.

mod helpers;
pub mod run;
mod structs;
mod types;

pub use structs::SidebarStory;
