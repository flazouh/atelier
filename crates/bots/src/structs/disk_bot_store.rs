use std::path::PathBuf;

/// Keeps bots, playbooks and notes as small JSON files in one folder. One process uses a folder at a time: two
/// that add a note at once may write the same number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskBotStore {
    pub(in super::super) root: PathBuf,
}
