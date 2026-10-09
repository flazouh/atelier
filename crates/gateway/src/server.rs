//! The listener and the checks a request passes before the protocol sees it: Host, Origin, the bearer token, the
//! verb and the path. Every connection gets its own thread and one answer.
mod helpers;
mod structs;

pub use structs::Gateway;
pub(crate) use structs::Shared;

#[cfg(test)]
mod tests;
