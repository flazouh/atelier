/// How much of a turn the brief keeps. A turn over budget steps down, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Detail {
    Whole,
    /// The ask and the tool lines, without the agent's words.
    Tools,
    /// The ask alone.
    Ask,
}

impl Detail {
    pub(super) const STEPS_DOWN: [Detail; 2] = [Detail::Tools, Detail::Ask];
}
