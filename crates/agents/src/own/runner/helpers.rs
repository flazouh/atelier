use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex, atomic::{Ordering}, mpsc::{channel}},
    thread,
    time::{Duration, Instant},
};

use atelier_project::Project;

use super::super::{
    OwnOptions,
    message::{Cancel, Model},
    permission::{Rules},
    store::{self, Meta},
    };
use crate::session::{EventSink, PermissionMode, Session, SessionError};
use super::structs::{Handle, Runner, Shared};
use super::types::COUNTER;

pub(super) fn new_id() -> String {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    format!("own-{millis:x}-{:x}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

pub(in super::super) fn open(
    model: Arc<dyn Model>,
    options: Arc<OwnOptions>,
    project: Arc<dyn Project>,
    request: crate::session::OpenRequest,
    sink: EventSink,
) -> Result<Box<dyn Session>, SessionError> {
    let (meta, messages) = match &request.resume {
        Some(id) => {
            let (meta, messages) = store::load(project.as_ref(), id.as_str())?;
            (meta, messages)
        }
        None => (Meta { id: new_id(), ..Meta::default() }, Vec::new()),
    };
    let chosen = request.model.clone().or_else(|| (!meta.model.is_empty()).then(|| meta.model.clone())).unwrap_or_else(|| options.default_model.clone());
    let mode = request.mode.unwrap_or(PermissionMode::Ask);
    let shared = Arc::new(Shared { cancel: Cancel::default(), mode: Mutex::new(mode), model: Mutex::new(chosen) });
    let (tx, rx) = channel();
    let runner = Runner {
        rules: Rules::new(options.allow.clone()),
        model,
        options,
        project,
        sink,
        shared: shared.clone(),
        rx,
        queue: VecDeque::new(),
        messages,
        meta,
        next_block: 0,
        closed: false,
        save_warned: false,
    };
    thread::Builder::new()
        .name("atelier-own-agent".into())
        .spawn(move || runner.run())
        .map_err(|e| SessionError::Start(e.to_string()))?;
    Ok(Box::new(Handle { tx, shared }))
}

/// Sleeps `total`, in slices, and stops early when `cancel` is set. `false` when it was.
pub(super) fn sleep(total: Duration, cancel: &Cancel) -> bool {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if cancel.is_set() {
            return false;
        }
        thread::sleep(Duration::from_millis(20).min(end.saturating_duration_since(Instant::now())));
    }
    !cancel.is_set()
}

pub(in super::super) fn root_name(root: &Path) -> String {
    root.file_name().and_then(|n| n.to_str()).unwrap_or("the project").to_string()
}
