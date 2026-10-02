use atelier_agents::commands::CommandInfo;

/// What the project holds for the lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lists {
    /// The project's skills and command files.
    pub project_commands: Vec<CommandInfo>,
    /// The project's files, by their paths.
    pub files: Vec<String>,
}
