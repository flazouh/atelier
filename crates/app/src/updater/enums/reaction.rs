/// What the window does besides keep the new state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reaction {
    Nothing,
    /// Tell the reader one line.
    Say(String),
}
