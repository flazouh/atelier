use super::super::Relaunch;

impl Relaunch {
    /// The two answers the question offers, in order: the restart, and the way out.
    pub const BUTTONS: [&'static str; 2] = ["Restart Anyway", "Cancel"];

    /// What a restart needs, given how many tabs hold unsaved edits.
    pub fn for_unsaved(unsaved: usize) -> Self {
        if unsaved == 0 { Relaunch::Go } else { Relaunch::Ask { unsaved } }
    }

    /// The question's first line: how many tabs would lose their edits.
    pub fn title(&self) -> String {
        match self {
            Relaunch::Go => String::new(),
            Relaunch::Ask { unsaved: 1 } => "1 tab has unsaved changes.".to_string(),
            Relaunch::Ask { unsaved } => format!("{unsaved} tabs have unsaved changes."),
        }
    }

    /// The question's second line: what a restart costs.
    pub fn detail(&self) -> &'static str {
        "They are lost if you restart."
    }
}
