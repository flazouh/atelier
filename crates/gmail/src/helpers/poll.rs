use std::{
    collections::HashMap,
    sync::{Arc, mpsc::Sender},
    time::{Duration, Instant},
};

use atelier_capabilities::{
    StopFlag,
    mail::{MailEvent, MailEventKind},
};

use super::map::row_summary;
use crate::{
    structs::{Row, SearchOut},
    traits::Runner,
    types::POLL_THREADS,
};

/// What the poll reads: the newest inbox threads.
pub(crate) const POLL_QUERY: &str = "in:inbox";

/// Sleeps `every`, in short steps so a dropped subscription stops the poll soon. False when it stopped.
fn wait(every: Duration, stop: &StopFlag) -> bool {
    let until = Instant::now() + every;
    while Instant::now() < until {
        if stop.is_stopped() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10).min(every));
    }
    !stop.is_stopped()
}

/// Reads the inbox every `every` and tells what changed since the last read: a thread not seen before or with a new date is
/// a `new_message`, a thread whose unread state changed is `changed`. The first rows are the baseline and tell nothing. A
/// read that fails is skipped; the next one tries again. The poll ends when the reader drops the subscription.
pub(crate) fn watch(
    runner: Arc<dyn Runner>,
    account: String,
    every: Duration,
    baseline: Vec<Row>,
    tx: Sender<MailEvent>,
    stop: StopFlag,
) {
    let mut seen: HashMap<String, (String, bool)> = baseline
        .into_iter()
        .map(|r| (r.thread_id, (r.date, r.unread)))
        .collect();
    let args: Vec<String> = [
        "search",
        POLL_QUERY,
        "-n",
        &POLL_THREADS.to_string(),
        "-json",
    ]
    .map(String::from)
    .to_vec();
    while wait(every, &stop) {
        let Ok(text) = runner.run(&args) else {
            continue;
        };
        let Ok(out) = serde_json::from_str::<SearchOut>(text.trim()) else {
            continue;
        };
        // Oldest first, so events come in the order the mail did.
        for row in out.rows.iter().rev() {
            let now = (row.date.clone(), row.unread);
            let kind = match seen.get(&row.thread_id) {
                None => Some(MailEventKind::NewMessage),
                Some(before) if before.0 != now.0 => Some(MailEventKind::NewMessage),
                Some(before) if *before != now => Some(MailEventKind::Changed),
                Some(_) => None,
            };
            seen.insert(row.thread_id.clone(), now);
            if let Some(kind) = kind
                && tx
                    .send(MailEvent {
                        kind,
                        thread: row_summary(&account, row),
                    })
                    .is_err()
            {
                return;
            }
        }
    }
}
