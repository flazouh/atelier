use atelier_agents::session::ToolKind;

/// How a call reads in its row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// "Ran", "Read", "Searched": past tense once it has ended, the "-ing" form while it runs.
    pub title: String,
    /// The argument that says which one, short: the command's first line, the pattern, the address.
    pub detail: Option<String>,
    /// The file the call is about, when it names one, which the row draws with its own icon.
    pub file: Option<String>,
    pub kind: ToolKind,
}
