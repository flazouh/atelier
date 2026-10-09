use serde::Deserialize;

/// What the platform updater tells the window, in the order it happens. The Mac's native side sends each as a line of
/// JSON.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateEvent {
    /// The updater started to look. `user` is whether the reader asked.
    Checking { user: bool },
    /// An update exists. The updater downloads it at once. `notes` is the changelog, in Markdown, when the feed carries it.
    Found { version: String, notes: Option<String>, user: bool },
    /// The download is `fraction` done, 0 to 1.
    Downloading { fraction: f64 },
    /// The download is unpacked, `fraction` done, 0 to 1.
    Extracting { fraction: f64 },
    /// The update is downloaded and waits for the reader to restart.
    Ready,
    /// The update installs, and the app is about to restart.
    Installing,
    /// There is no newer version.
    UpToDate,
    Failed { message: String },
    /// The update ended with nothing more to show.
    Idle,
}
