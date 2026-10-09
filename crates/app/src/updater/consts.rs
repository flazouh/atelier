/// What a build that cannot update says when the reader asks for an update.
pub const UNAVAILABLE_NOTICE: &str = "This build does not update itself. Updates come with the released app.";

/// What the reader reads after a no to the restart: the update is kept, and does not install over unsaved edits.
pub const UPDATE_WAITS_NOTICE: &str = "The update waits. Save your edits, then check for updates again.";

/// What a look the reader asked for says when it finds nothing newer.
pub const UP_TO_DATE_NOTICE: &str = "atelier is up to date.";

/// What the reader reads when the look starts.
pub const CHECKING_NOTICE: &str = "Looking for updates…";

/// What the reader reads when asking again while the update waits for the restart.
pub const UPDATE_READY_NOTICE: &str = "The update is ready. Press the Update button in the title bar to restart.";

/// What the reader reads when asking again while the update downloads.
pub const DOWNLOADING_NOTICE: &str = "The update is downloading.";

/// The way the platform updater asks the window for a restart.
pub type RequestSender = futures_channel::mpsc::UnboundedSender<Box<dyn super::RelaunchRequest>>;

/// The window's end of that way.
pub type Requests = futures_channel::mpsc::UnboundedReceiver<Box<dyn super::RelaunchRequest>>;

/// The way the platform updater tells the window how an update goes.
pub type UpdateSender = futures_channel::mpsc::UnboundedSender<super::UpdateEvent>;

/// The window's end of that way.
pub type UpdateEvents = futures_channel::mpsc::UnboundedReceiver<super::UpdateEvent>;
