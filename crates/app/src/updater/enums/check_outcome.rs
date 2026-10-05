/// What asking for an update did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// The updater started looking, and shows its own window.
    Started,
    /// This build cannot update itself; nothing started.
    Unavailable,
}
