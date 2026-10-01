//! What the reader has seen, kept on this machine. **Reviewed State** is per file version: a file is seen
//! for the version (the blob) it had when it was marked, so a push that changes the file makes it unseen
//! again, and a push that leaves it alone does not. It is a small SQLite database in the app's data
//! folder, the tracker's pattern: migrations by `user_version`, WAL.
use std::{collections::HashMap, path::Path, sync::Mutex};

use atelier_forge::PullRef;
use rusqlite::{Connection, params};

const STEPS: &[&str] = &["CREATE TABLE reviewed (
        host TEXT NOT NULL,
        repo TEXT NOT NULL,
        number INTEGER NOT NULL,
        path TEXT NOT NULL,
        version TEXT NOT NULL,
        at INTEGER NOT NULL,
        PRIMARY KEY (host, repo, number, path)
    );",
    // 2: when the reader last opened a pull request, for "unread" in the list.
    "CREATE TABLE opened (
        host TEXT NOT NULL,
        repo TEXT NOT NULL,
        number INTEGER NOT NULL,
        at INTEGER NOT NULL,
        PRIMARY KEY (host, repo, number)
    );"];

pub struct Reviewed {
    conn: Mutex<Connection>,
}

fn open(mut conn: Connection) -> rusqlite::Result<Reviewed> {
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

impl Reviewed {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        open(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        open(Connection::open_in_memory()?)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The reader has read `path` as it is in `version`. `at` is the time, in seconds.
    pub fn mark(&self, pull: &PullRef, path: &str, version: &str, at: u64) -> rusqlite::Result<()> {
        self.conn()
            .prepare_cached("INSERT INTO reviewed (host, repo, number, path, version, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (host, repo, number, path) DO UPDATE SET version = ?5, at = ?6")?
            .execute(params![pull.repo.host, pull.repo.slug(), pull.number as i64, path, version, at as i64])?;
        Ok(())
    }

    pub fn unmark(&self, pull: &PullRef, path: &str) -> rusqlite::Result<()> {
        self.conn()
            .prepare_cached("DELETE FROM reviewed WHERE host = ?1 AND repo = ?2 AND number = ?3 AND path = ?4")?
            .execute(params![pull.repo.host, pull.repo.slug(), pull.number as i64, path])?;
        Ok(())
    }

    /// Every file marked on the pull request, with the version it was marked at.
    pub fn marks(&self, pull: &PullRef) -> rusqlite::Result<HashMap<String, String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT path, version FROM reviewed WHERE host = ?1 AND repo = ?2 AND number = ?3")?;
        let rows = stmt.query_map(params![pull.repo.host, pull.repo.slug(), pull.number as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect()
    }

    /// The reader opened the pull request at `at`.
    pub fn opened(&self, pull: &PullRef, at: u64) -> rusqlite::Result<()> {
        self.conn()
            .prepare_cached("INSERT INTO opened (host, repo, number, at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (host, repo, number) DO UPDATE SET at = ?4")?
            .execute(params![pull.repo.host, pull.repo.slug(), pull.number as i64, at as i64])?;
        Ok(())
    }

    /// When each pull request was last opened, by `(repo slug, number)`.
    pub fn opened_at(&self) -> rusqlite::Result<HashMap<(String, u64), u64>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached("SELECT repo, number, at FROM opened")?;
        let rows = stmt.query_map([], |r| Ok(((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64), r.get::<_, i64>(2)? as u64)))?;
        rows.collect()
    }

    /// Puts every file back to unseen ("Put all back").
    pub fn clear(&self, pull: &PullRef) -> rusqlite::Result<usize> {
        self.conn()
            .prepare_cached("DELETE FROM reviewed WHERE host = ?1 AND repo = ?2 AND number = ?3")?
            .execute(params![pull.repo.host, pull.repo.slug(), pull.number as i64])
    }
}

/// Whether a file at `version` is seen, given the marks.
pub fn is_seen(marks: &HashMap<String, String>, path: &str, version: &str) -> bool {
    !version.is_empty() && marks.get(path).is_some_and(|marked| marked == version)
}
