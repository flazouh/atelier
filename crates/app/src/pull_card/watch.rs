//! One watch per pull request, shared by every card that shows it: it reads the pull request and its
//! checks off the UI thread at the pace `poll` sets. It reads only while a card of it is drawn in an
//! active window: each card tells it when it draws, and a wait that no draw followed pauses it. The next
//! draw in an active window reads at once.

mod helpers;
mod structs;
mod types;

pub use helpers::watch;
pub use structs::PullWatch;
