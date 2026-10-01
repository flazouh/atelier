/// The side panes' widths the reader asked for; `None` for a pane that is hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wants {
    pub sidebar: Option<f32>,
    pub right: Option<f32>,
}

/// The widths the panes get; `None` for a pane that does not show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widths {
    pub sidebar: Option<f32>,
    pub agent: f32,
    pub right: Option<f32>,
}
