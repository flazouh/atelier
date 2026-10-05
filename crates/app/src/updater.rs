//! Updates for the packaged app. The platform's own updater (Sparkle on the Mac) looks for a release, downloads it,
//! as a small patch when one fits, and installs it. This module is the app's side of that: the Check for Updates
//! action, a build that cannot update, and the question the app asks before an update restarts it while a tab holds
//! unsaved edits. [`UpdateDriver`] is the platform updater; [`RelaunchRequest`] is its wait for the app's answer.
mod consts;
mod enums;
mod helpers;
mod impls;
mod structs;
mod traits;
pub use consts::{RequestSender, Requests, UNAVAILABLE_NOTICE, UPDATE_WAITS_NOTICE};
pub use helpers::driver;
pub use enums::CheckOutcome;
pub use structs::{Held, NoDriver, Question, Updater};
#[cfg(target_os = "macos")]
pub use structs::{NativeRelaunch, SparkleDriver};
pub use traits::{RelaunchRequest, UpdateDriver};
#[cfg(test)]
pub(crate) mod fakes;
#[cfg(test)]
mod tests;
