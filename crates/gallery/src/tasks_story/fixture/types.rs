/// The time the fixture counts from, in seconds since the Unix epoch.
pub const BASE: u64 = 1_790_700_000;

/// Who the reader is.
pub const ME: &str = "Alex";

pub(super) const VERBS: [&str; 12] = ["Add", "Fix", "Port", "Remove", "Speed up", "Rewrite", "Document", "Test", "Split", "Move", "Rename", "Check"];

pub(super) const THINGS: [&str; 16] = [
    "the sidebar",
    "the diff layout",
    "the review queue",
    "the forge client",
    "the highlight cache",
    "session restore",
    "the task board",
    "the finder",
    "the theme picker",
    "the SSH reconnect",
    "the PR chip",
    "the composer toolbar",
    "the keyboard table",
    "the settings file",
    "the release build",
    "the markdown parser",
];

pub(super) const TAILS: [&str; 6] = ["", " on the Mac", " for long lines", " after a resize", " with no network", " in dark mode"];
