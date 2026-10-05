use super::super::Question;

impl Question {
    /// The two answers the question offers, in order: the restart, and the way out.
    pub const BUTTONS: [&'static str; 2] = ["Restart Anyway", "Cancel"];

    /// The question to put to the reader, when `unsaved` tabs hold edits that are not saved; none when none do.
    pub fn about(unsaved: usize) -> Option<Self> {
        (unsaved > 0).then_some(Question { unsaved })
    }

    /// The first line: how many tabs would lose their edits.
    pub fn title(&self) -> String {
        match self.unsaved {
            1 => "1 tab has unsaved changes.".to_string(),
            unsaved => format!("{unsaved} tabs have unsaved changes."),
        }
    }

    /// The second line: what a restart costs.
    pub fn detail(&self) -> &'static str {
        "They are lost if you restart."
    }
}
