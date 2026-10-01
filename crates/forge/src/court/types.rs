/// Who owes the next move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Court {
    NeedsYou,
    Waiting,
    Running,
    Settled,
}

impl Court {
    /// Reading order, most urgent first.
    pub const ALL: [Court; 4] = [Self::NeedsYou, Self::Waiting, Self::Running, Self::Settled];

    pub(super) fn urgency(self) -> usize {
        Self::ALL.iter().position(|court| *court == self).unwrap_or(usize::MAX)
    }
}
