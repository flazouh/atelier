use std::time::Duration;

/// How often the live clock ticks.
pub const LIVE_STEP: Duration = Duration::from_millis(900);

/// The ticks in one pass of the script. It starts over after this.
pub(super) const LOOP: usize = 14;

pub(super) const TOOLS: [&str; 5] = [
    "Read crates/ui/src/theme.rs",
    "Grep hunk_starts",
    "Read crates/ui/src/file_diff.rs",
    "Edit crates/ui/src/file_diff.rs",
    "Bash cargo test -p ui",
];

pub const PR_TEXT: &str = "This fixes the bug from #3344: the header line no longer counts. The failure in \
#9999 is unrelated, and `git show #3344` in code stays plain.";
