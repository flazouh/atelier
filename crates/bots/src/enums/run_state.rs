/// Where a run is. It is read from the steps each time, so the two cannot disagree. Only a stop is kept with the
/// run, because no step can say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    /// A step is ready to work, or at work.
    Working,
    NeedsPerson,
    /// Every step is done or skipped.
    Done,
    /// A step failed. A person can try it again, skip it or stop the run.
    Failed,
    /// A person stopped the run. Nothing changes it after that.
    Stopped,
}
