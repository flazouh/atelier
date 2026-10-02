//! Server-sent events, from bytes that arrive in any pieces: a line may split anywhere, even inside a
//! UTF-8 character. Only the `event` and `data` fields matter to the model APIs.

mod structs;
mod types;

pub use structs::{Parser, SseEvent};
