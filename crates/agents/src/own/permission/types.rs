#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Ask the reader and wait.
    Ask,
    /// Refuse. The reason goes to the model, which reads it and goes on.
    Deny(String),
}
