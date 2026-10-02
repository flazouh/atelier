use crate::data::PartKind;

/// The parts of a refresh: the ones that change while people work. The header, the threads, the remarks,
/// the checks and the unsent comments.
pub const LIVE: [PartKind; 5] = [PartKind::Pull, PartKind::Threads, PartKind::Remarks, PartKind::Checks, PartKind::Held];
