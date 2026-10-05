use super::super::{Question, RelaunchRequest};

/// The updater's wait for a restart, held while the reader decides whether unsaved edits may be lost.
pub struct Held {
    pub(in super::super) request: Option<Box<dyn RelaunchRequest>>,
    pub(in super::super) question: Question,
}
