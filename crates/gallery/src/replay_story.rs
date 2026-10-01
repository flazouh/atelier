//! The "Agent replay" story: a captured `claude` run played back through the Claude Code mapper into a
//! `Conversation`, drawn with the agent panel's own parts. It is the path a live session takes, minus the
//! process: lines become events, events go through the queue that coalesces them, and the panel draws what
//! the conversation holds. `REPLAY_FIXTURE=<name>` picks the run (`subagent_foreground` by default);
//! `REPLAY_LINES` sets how many lines play each frame (1 by default).

mod helpers;
mod structs;
mod types;

pub use structs::ReplayStory;
