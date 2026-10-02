/// The most files `@` offers: past this, a huge repository would cost more than the list is worth.
pub(super) const MOST_FILES: usize = 20_000;

/// The atelier commands that do something today. A command atelier lists but cannot run yet stays out.
pub(super) const RUNNING: [&str; 3] = ["files", "tasks", "review"];
