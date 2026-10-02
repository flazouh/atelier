//! Changes atelier asks GitHub to make. Each is one mutation, or REST where GraphQL has none. Nothing
//! here retries a write on its own: a change that may have landed is never sent twice. The client
//! retries a rate limit, which GitHub refuses before it acts, and a read that met a server error.

mod helpers;
mod impls;
mod structs;
mod types;

#[cfg(test)]
pub(super) use helpers::encode_ref;
