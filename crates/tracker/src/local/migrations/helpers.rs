use rusqlite::Connection;

use super::types::{CURRENT, STEPS};
use crate::{TrackerError, TrackerResult};

/// Applies the steps the database has not had, each in its own transaction. A database written by a newer
/// build is refused rather than misread.
pub fn run(conn: &mut Connection) -> TrackerResult<()> {
    let version: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > CURRENT {
        return Err(TrackerError::Storage(format!(
            "this task database is version {version}, and this atelier knows up to {CURRENT}: update atelier"
        )));
    }
    for (index, step) in STEPS.iter().enumerate().skip(version) {
        let tx = conn.transaction()?;
        tx.execute_batch(step)?;
        tx.pragma_update(None, "user_version", index + 1)?;
        tx.commit()?;
    }
    Ok(())
}
