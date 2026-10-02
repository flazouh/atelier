use std::time::Duration;

pub(super) const FRAME: Duration = Duration::from_millis(33);

/// Which face the made-up session drives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Face {
    Bar,
    Composer,
}
