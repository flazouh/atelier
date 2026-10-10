//! The "Bots" story: the seven robots of `docs/bots/faces.v1.json`, many at once, in any mood. It prints how long
//! building and submitting their paths takes, so the player can be judged by a number. `BOTS_COUNT`, `BOTS_MOOD`
//! and `BOTS_LOG` (frames between two log lines) set the load without a click.

mod structs;

pub use structs::BotStory;
