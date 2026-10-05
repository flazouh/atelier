/// What a build that cannot update says when the reader asks for an update.
pub const UNAVAILABLE_NOTICE: &str = "This build does not update itself. Updates come with the released app.";

/// The way the platform updater asks the window for a restart.
pub type RequestSender = futures_channel::mpsc::UnboundedSender<Box<dyn super::RelaunchRequest>>;

/// The window's end of that way.
pub type Requests = futures_channel::mpsc::UnboundedReceiver<Box<dyn super::RelaunchRequest>>;
