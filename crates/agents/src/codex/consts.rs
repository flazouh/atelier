use atelier_ui::Strip;

/// The name a session records for Codex.
pub const BACKEND: &str = "codex";

/// `npx` finds the adapter, or fetches it once: nothing to install first beyond Node.
pub(super) const PROGRAM: &str = "npx";
pub(super) const ADAPTER: [&str; 2] = ["-y", "@agentclientprotocol/codex-acp"];

/// What signs Codex in: the adapter has no login of its own, and shares `~/.codex/auth.json` with this command.
pub(super) const LOGIN: [&str; 2] = ["codex", "login"];

/// Codex's mark, held still: Codex has no animated mark.
pub(super) const KNOT: Strip = Strip {
    path: "coding/codex-dark.svg",
    bytes: include_bytes!("../../assets/coding/codex-dark.svg"),
    frames: 1,
    frame_ms: 1000,
    loops: false,
};

/// A neutral grey that reads on light and dark pages: Codex's own mark is black or white.
pub(super) const GREY: u32 = 0x8C8A84;

/// The lighter grey that walks across the label.
pub(super) const GLIMMER: u32 = 0xBDBAB2;
