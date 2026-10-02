//! atelier's own record of a session: with no external store, the conversation is kept in the project's data
//! folder (`Project::data_write`), outside the repository and on the project's host, so it never shows in
//! `git status`, in `agent/sessions/`. Sessions written by an older version (named lathe then) into the
//! project's own `.lathe/agent/sessions/` are moved there the first time the list is read ([`migrate`]).
//! Two files per session: `<id>.jsonl` holds one message a line, and `<id>.meta` holds the title and the
//! time, small enough to read for a list. `history` turns the messages into the events a UI shows.

mod helpers;
mod structs;
mod types;

pub use helpers::{events_of, history, list, load, migrate, now, save, title_of, valid_id};
pub use structs::Meta;
pub use types::DIR;
