use std::collections::HashMap;

use gpui_kit::{Global, Task, WeakEntity};

use crate::open_project::OpenProject;

/// A project's reads for its open chip cards, by pull request number.
#[derive(Default)]
pub struct Reading {
    /// The read loop of each open card; dropping it stops the reads.
    pub(crate) open: HashMap<u64, Task<()>>,
    /// The last pull request read, for Merge's head and method.
    pub(super) pulls: HashMap<u64, atelier_forge::Pull>,
    /// The line each failed job's log gave, by job id, so a live read fetches no log twice.
    pub(super) lines: HashMap<u64, Option<String>>,
}

/// The projects open in this app, so a card finds the one whose repository it names.
#[derive(Default)]
pub(super) struct Projects(pub(super) Vec<WeakEntity<OpenProject>>);

impl Global for Projects {}
