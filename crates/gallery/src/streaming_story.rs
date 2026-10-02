//! The "Streaming" story: an answer streams in at 60 tokens a second, over and over, in the app's own `AgentText`.
//! `STREAM_FADE=0` turns the tail's fade off, to set the old look beside the new. With `ATELIER_FRAMES`-style timing the
//! frame cost of both is in `cargo test -p ui streaming_frame_cost -- --ignored --nocapture`.

mod helpers;
mod structs;
mod types;

pub use structs::StreamingStory;
