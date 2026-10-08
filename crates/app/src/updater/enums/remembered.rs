/// What to do with the changelog a past run kept, when this version starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remembered {
    /// It is for the version that runs: show it once.
    Show,
    /// It is for a version that is not installed yet: keep it.
    Wait,
    /// It is for an older version, or unreadable: drop it.
    Forget,
}
