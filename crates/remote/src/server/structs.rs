use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::{AtomicU64}, mpsc},
};

use atelier_project::{Control, LocalProject, Watch};

use crate::protocol::Pid;

/// One process the host runs for the app.
pub(super) struct Running {
    /// Feeds its stdin in order; dropping it closes stdin.
    pub(super) input: Option<mpsc::Sender<Vec<u8>>>,
    pub(super) control: Arc<Mutex<Box<dyn Control>>>,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) project: Mutex<Option<Arc<LocalProject>>>,
    pub(super) running: Mutex<HashMap<Pid, Running>>,
    pub(super) next_pid: AtomicU64,
    pub(super) watch: Mutex<Option<Watch>>,
    /// Where projects keep their data folders; `None` for the host's own data folder.
    pub(super) data_dir: Option<PathBuf>,
}
