use super::Window;

/// What a provider said of its allowance once.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    pub windows: Vec<Window>,
    /// A line for the hover: spend beyond the plan, the plan's name.
    pub note: Option<String>,
}
