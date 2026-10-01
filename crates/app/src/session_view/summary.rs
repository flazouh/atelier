//! What a tool call is about, in words a reader takes in at a glance: a title ("Ran", "Read", "Searched"), the
//! argument that says which (the command, the file, the pattern) and an icon for its kind. The agent gives a call a
//! name and a bag of arguments; "Bash" alone says nothing, but "Ran `cargo test -p atelier-ui`" does. Pure.

mod helpers;
mod structs;
mod types;

pub use helpers::summary;

#[cfg(test)]
mod tests;
