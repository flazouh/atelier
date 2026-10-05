/// The platform's updater: it looks for a release, downloads it and installs it.
pub trait UpdateDriver {
    /// Whether this build can update itself. A released app can; a build from source cannot.
    fn available(&self) -> bool;
    /// Looks for an update now, and shows the updater's own window with the answer.
    fn check(&self);
}
