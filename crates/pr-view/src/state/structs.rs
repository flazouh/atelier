use std::{collections::HashMap, path::Path, sync::Mutex};

use atelier_forge::PullRef;
use rusqlite::{Connection, params};

use super::helpers::open;

pub struct Reviewed {
    pub(super) conn: Mutex<Connection>,
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

    pub(super) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
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
