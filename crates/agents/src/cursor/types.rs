use atelier_ui::Strip;

/// The name `agent acp` reports and a session records.
pub const BACKEND: &str = "cursor";

/// Cursor's cube, held still: Cursor has no animated mark.
pub(super) const CUBE: Strip = Strip {
    path: "coding/cursor-dark.svg",
    bytes: include_bytes!("../../assets/coding/cursor-dark.svg"),
    frames: 1,
    frame_ms: 1000,
    loops: false,
};

/// A neutral grey that reads on light and dark pages: Cursor's own mark is black or white.
pub(super) const GREY: u32 = 0x8C8A84;

/// The lighter grey that walks across the label.
pub(super) const GLIMMER: u32 = 0xBDBAB2;
