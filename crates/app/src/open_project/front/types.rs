/// What the right pane holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Front {
    #[default]
    Editor,
    Pulls,
    Tasks,
}
