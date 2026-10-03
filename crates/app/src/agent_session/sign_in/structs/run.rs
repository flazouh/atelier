use std::sync::{Arc, atomic::AtomicBool};

use gpui_kit::Task;

/// A sign-in command that runs. Dropping it stops the command, so a closed session leaves no browser wait behind.
pub struct Run {
    pub(in super::super) cancelled: Arc<AtomicBool>,
    pub(in super::super) _task: Task<()>,
}
