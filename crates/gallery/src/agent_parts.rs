//! Stories for the agent panel parts, and the fixtures the Agent panel story shares with them.
//!
//! `tick` is the live clock: `GALLERY_LIVE=1`, or Play live in a story, advances it every
//! [`LIVE_STEP`], so the live tool call, the tool count, the strip's rows, and the arriving files move.
//! With it still, every story shows one fixed frame.

mod helpers;
mod types;

pub use helpers::{
    anthropic, changed_files, changed_files_story, model_badge_story, pr_3344, pr_card_story,
    pr_chip_story, resolve_pr, running_card, strip_rows, subagent_card_story,
    subagent_strip_story,
};
pub use types::{LIVE_STEP, PR_TEXT};
