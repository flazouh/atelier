use crate::{Output, RunError};

/// Runs one `slackcli` command and gives back what it printed. The provider builds the arguments and reads the answer; a
/// test gives a fake that behaves like a small Slack, so no test reaches a real workspace.
pub trait Runner: Send + Sync {
    /// `args` are the arguments after the program name, as separate strings, so no shell reads them.
    fn run(&self, args: &[String]) -> Result<Output, RunError>;
}
