use super::super::structs::BotId;

/// Where a note is kept. The three layers of the spec: the bot's own, a workspace's, a project's. Each layer
/// stays where it was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryScope {
    Bot(BotId),
    /// A workspace, by its id.
    Workspace(String),
    /// A project, by its folder.
    Project(String),
}
