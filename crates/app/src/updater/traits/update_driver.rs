/// The platform's updater: it looks for a release, downloads it and installs it. It tells the window how that goes with
/// [`UpdateEvent`](super::UpdateEvent)s, and the window shows it in its own look.
pub trait UpdateDriver {
    /// Whether this build can update itself. A released app can; a build from source cannot.
    fn available(&self) -> bool;
    /// Looks for an update now. `asked` is whether the reader asked for the look.
    fn check(&self, asked: bool);
    /// The reader chose to restart: the update, downloaded already, installs.
    fn install(&self);
    /// The reader chose to wait: the update installs when the app quits.
    fn later(&self);
}
