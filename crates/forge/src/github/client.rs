//! One request with the forge's manners: back off when it asks, retry a server that stumbles, follow
//! pages, and turn what goes wrong into a [`ForgeError`] the UI can show.

mod helpers;
mod impls;
mod structs;
mod types;

pub(super) use structs::Client;

#[cfg(test)]
mod tests;
