//! A backend for tests, in the test's thread: each message plays the next scripted turn into the sink,
//! after the scripted work on the files, and every command is kept.

mod helpers;
mod structs;

pub use helpers::{backend_with_history, ended, fake_agent, named_agent, git_project, git_project_in, scripted_agent, start, start_in, start_forking, start_on_providers, start_shown_in, start_signing_in};
pub use structs::Fake;
