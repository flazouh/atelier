use super::Entry;

/// What the bots folder holds, as read last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    /// Every bot, in the order of their ids.
    pub entries: Vec<Entry>,
    /// Why the folder could not be read, in words that name the file. The view shows it in place of the bots.
    pub error: Option<String>,
}
