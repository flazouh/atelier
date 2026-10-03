/// One span a provider limits: Claude's five hours, Codex's thirty days.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// Short, for the status bar: `5h`, `7d`.
    pub label: String,
    /// How much of it is used, 0 to 1.
    pub used: f32,
    /// Seconds until it resets, when the provider says.
    pub resets_in: Option<u64>,
}
