use std::{collections::HashMap, sync::Mutex};

use rusqlite::Connection;

use super::structs::Reviewed;
use super::types::STEPS;

pub(super) fn open(mut conn: Connection) -> rusqlite::Result<Reviewed> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let _ = conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()));
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    let version: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > STEPS.len() {
        return Err(rusqlite::Error::InvalidParameterName(format!("this database is version {version}, and this atelier knows up to {}: update atelier", STEPS.len())));
    }
    for (index, step) in STEPS.iter().enumerate().skip(version) {
        let tx = conn.transaction()?;
        tx.execute_batch(step)?;
        tx.pragma_update(None, "user_version", index + 1)?;
        tx.commit()?;
    }
    Ok(Reviewed { conn: Mutex::new(conn) })
}

/// Whether a file at `version` is seen, given the marks.
pub fn is_seen(marks: &HashMap<String, String>, path: &str, version: &str) -> bool {
    !version.is_empty() && marks.get(path).is_some_and(|marked| marked == version)
}
