//! Reviewing what an agent did. For each turn, the text of every file the agent touches, taken before its
//! edit lands ([`TurnTracker`]); at the turn's end, the hunks between that text and the file now
//! ([`TurnReview`], one [`Merged`] per file, in the form the inline review edits); the turns of a session
//! ([`SessionReview`]); what the user has read ([`Reviewed`]); and the comments that go back to the agent
//! ([`Comments`]). Everything is pure text and git, tested without a window. `docs/review.md` says how
//! the UI uses it.
mod comments;
mod file_review;
mod git_state;
mod lines;
mod merged;
pub mod place;
pub mod present;
mod reviewed;
mod tracker;
mod turn;
mod words;

pub use comments::{Comments, ReviewComment};
pub use file_review::{Change, Content, FileReview};
pub use merged::{Anchor, Merged, Side};
pub use reviewed::Reviewed;
pub use tracker::TurnTracker;
pub use turn::{SessionReview, TurnReview};
pub use words::{RowChange, pair_rows, word_changes};
