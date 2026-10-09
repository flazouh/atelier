//! The pure parts of the provider: reading Gmail's dates, building queries, mapping failures and rows, and the inbox poll.
mod date;
mod failure;
mod map;
mod poll;
mod query;
mod shell;

#[cfg(test)]
pub(crate) use date::parse_display_date;
pub(crate) use failure::map_failure;
pub(crate) use map::{
    attachment_at, label_mailbox, row_summary, sanitize_filename, thread_from_wire, valid_id,
};
pub(crate) use poll::{POLL_QUERY, watch};
pub(crate) use query::build_query;
pub(crate) use shell::remote_command;
#[cfg(test)]
pub(crate) use shell::shell_quote;
