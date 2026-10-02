//! The batched lookup: many pull numbers of one repository in one request, each under its own alias.

mod helpers;
mod types;

pub(super) use helpers::{briefs, open_for};
