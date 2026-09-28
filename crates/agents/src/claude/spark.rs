//! Claude's spark: the animated mark Claude shows while it works, as beui [`Strip`]s.
//!
//! The strips are Anthropic's artwork, used here for our own Claude Code front end. They come from the
//! Claude desktop app 2.9939.2, `/Applications/Claude.app/Contents/Resources/ion-dist/assets/v1/
//! cf2613ee5-Btwr9m9F.js` (read on 2026-09-26), where each state is `{svg, width: 100, height: 100,
//! frameCount, speed}`. Each SVG stacks its frames top to bottom, so its `viewBox` is 100 wide and
//! `100 * frames` tall; entrance and exit are exported at 100x601 and 101x601, which beui's
//! [`beui::Sprite`] allows for. The app (component `ob` in `shared-frame-BgE9BXIr.js`) slides the strip
//! up with `steps(frames, jump-none)`, as [`beui::Sprite`] does.

use beui::Strip;

macro_rules! strips {
    ($($variant:ident => $file:literal, $frames:literal, $frame_ms:literal, $loops:literal),* $(,)?) => {
        /// One of the spark's animations. Looping states play until the state changes; the others play
        /// once and hold their last frame.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum SparkState { $($variant),* }

        impl SparkState {
            pub const ALL: &[SparkState] = &[$(Self::$variant),*];

            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $file),* }
            }

            /// The state's strip: frame count, time per frame, and whether it loops, from the desktop
            /// app's table.
            pub fn strip(self) -> Strip {
                match self {
                    $(Self::$variant => Strip {
                        path: concat!("claude/spark/", $file, ".svg"),
                        bytes: include_bytes!(concat!("../../assets/claude/spark/", $file, ".svg")),
                        frames: $frames,
                        frame_ms: $frame_ms,
                        loops: $loops,
                    }),*
                }
            }
        }
    };
}

strips! {
    Thinking => "thinking", 9, 90, true,
    Writing => "writing", 8, 90, true,
    Waiting => "waiting", 16, 600, true,
    Shimmer => "shimmer", 15, 100, true,
    Orbiting => "orbiting", 18, 100, true,
    Entrance => "entrance", 6, 70, false,
    Exit => "exit", 6, 70, false,
    Tickle => "tickle", 7, 40, false,
}

#[cfg(test)]
mod tests;
