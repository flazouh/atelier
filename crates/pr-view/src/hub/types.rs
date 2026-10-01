use atelier_forge::PullRef;

/// What the hub tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum PrEvent {
    /// The reader wants a file in the editor: `path` at `line`, as it is at the pull request's head.
    OpenFile { pull: PullRef, path: String, line: Option<u32> },
    /// The reader wants the session linked to this pull request (see [`PrHub::set_linked_session`]).
    OpenSession(PullRef),
    /// The reader went back from a pull request to the list.
    Closed(PullRef),
}
