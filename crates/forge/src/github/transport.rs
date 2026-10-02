//! How a request reaches the forge. The rest of the GitHub backend sees only this: a method, a path
//! and a body in, a status, headers and a body out. Nothing here holds a credential.

mod structs;
mod traits;
mod types;

pub use structs::{Reply, Request};
pub use traits::Transport;
pub use types::TransportError;
