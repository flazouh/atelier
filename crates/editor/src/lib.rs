//! A file's editor wired to its language server, shared by the app and the gallery.

pub mod session;

pub use session::{ASK, EditorSession, Elsewhere, Jump, READY, file_name, go_to_definition};
