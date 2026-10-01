use atelier_forge::PullRef;

pub enum CardEvent {
    /// The reader pressed the card: show the pull request.
    Show(PullRef),
}
