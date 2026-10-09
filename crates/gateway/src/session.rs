//! The identity of one agent session: the address to call and the token that proves who is calling.
mod helpers;
mod structs;

pub(crate) use helpers::random_hex;
pub use structs::SessionAccess;
