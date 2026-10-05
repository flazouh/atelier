/// What the reader is asked before an update restarts the app: tabs hold edits that are not saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Question {
    pub(in super::super) unsaved: usize,
}
