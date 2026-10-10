//! The functions of the crate.

mod bot;
mod bot_id;
mod bots_error;
mod disk_bot_store;
mod disk_bot_store_files;
mod playbook;
mod starter_crew;

pub use starter_crew::{seed_starters, starter_crew, starter_playbooks};
