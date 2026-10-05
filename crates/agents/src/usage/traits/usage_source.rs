use atelier_project::Project;

use super::super::structs::Reading;

/// One provider's allowance, read where its sign-in is.
pub trait UsageSource: Send + Sync {
    /// The provider's name, as the status bar shows it.
    fn name(&self) -> &str;

    /// What is used now. `now` is the Unix time in seconds, so a reset is told as the time left. The reason is in
    /// words for the reader: not signed in, offline, a sign-in that ran out.
    fn read(&self, project: &dyn Project, now: i64) -> Result<Reading, String>;
}
