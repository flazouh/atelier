use super::types::CommandSource;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    /// Without the slash.
    pub name: String,
    pub source: CommandSource,
    /// One line of what it does.
    pub summary: String,
    /// What goes after the name, such as "<what the session is for>".
    pub args_hint: Option<String>,
}

impl CommandInfo {
    pub(super) fn new(name: &str, source: CommandSource, summary: &str, args_hint: Option<&str>) -> Self {
        Self { name: name.into(), source, summary: summary.into(), args_hint: args_hint.map(Into::into) }
    }
}
