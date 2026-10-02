//! The seam between atelier and an agent. A backend starts sessions; a session takes commands and
//! pushes events into a sink. Neither says how: a session may hold a process, a socket or a loop that
//! runs in this process.

mod structs;
mod traits;
mod types;

pub use structs::{Capabilities, ModelChoice, OpenRequest, SessionSummary};
pub use traits::{Backend, Session};
pub use types::{ApiKey, EventSink, Provider, SessionError};
