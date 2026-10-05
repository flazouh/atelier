/// Whether an update may restart the app now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relaunch {
    /// Nothing is unsaved: restart at once.
    Go,
    /// Tabs hold unsaved edits: ask first.
    Ask { unsaved: usize },
}
