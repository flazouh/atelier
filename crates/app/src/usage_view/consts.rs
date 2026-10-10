/// The longest range the dashboard shows. The logs are read for this many days once; a shorter range is a query.
pub const LONGEST: u32 = 30;
/// How many sessions the list shows, and how many models.
pub const SESSIONS: usize = 8;
pub const MODELS: usize = 6;
/// The hue of each provider in the theme's chart palette.
pub const CLAUDE_HUE: usize = 0;
pub const CODEX_HUE: usize = 1;
pub const OPENROUTER_HUE: usize = 2;
/// How many shades of one hue the palette has: several accounts of one provider take them in turn.
pub const SHADES: usize = 3;
/// What the dashboard says when there is nothing to draw.
pub const NOTHING: &str = "No usage found yet. Atelier reads the session logs of Claude Code and Codex on this computer.";
pub const READING: &str = "Reading the session logs…";
pub const NO_LOGS: &str = "This source keeps no token logs here. Its limit is in its tile.";
/// The room the detail leaves round its content, in design pixels.
pub const DETAIL_PAD: f32 = 24.;
