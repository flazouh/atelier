/// One piece of a shell line.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Token {
    /// A word, and whether the shell would expand it (a variable, a glob, `~`, a substitution): then its text is not its path.
    Word { text: String, expands: bool },
    /// A control or redirect operator: `;`, `|`, `&&`, `>`, `>>`, `&>`, `<<` and the rest.
    Op(String),
}
