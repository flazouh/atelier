/// What a build that cannot update says when the reader asks for an update.
pub const UNAVAILABLE_NOTICE: &str = "This build does not update itself. Updates come with the released app.";

/// What the reader reads after a no to the restart: the update is kept, and does not install over unsaved edits.
pub const UPDATE_WAITS_NOTICE: &str = "The update waits. Save your edits, then check for updates again.";

/// The way the platform updater asks the window for a restart.
pub type RequestSender = futures_channel::mpsc::UnboundedSender<Box<dyn super::RelaunchRequest>>;

/// The window's end of that way.
pub type Requests = futures_channel::mpsc::UnboundedReceiver<Box<dyn super::RelaunchRequest>>;
