use super::super::structs::{Reading, Window};

impl Reading {
    /// Adds a window `percent` used (0 to 100; any other is held in it).
    pub(in super::super) fn window(mut self, label: impl Into<String>, percent: f64, resets_in: Option<u64>) -> Self {
        let used = if percent.is_finite() { (percent / 100.).clamp(0., 1.) as f32 } else { 0. };
        self.windows.push(Window { label: label.into(), used, resets_in });
        self
    }

    pub(in super::super) fn note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }
}
