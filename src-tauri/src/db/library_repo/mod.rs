//! Watched roots and the whole-library maintenance reads. Separate from
//! `TrackRepository` because none of this is on the per-page read path.

mod maintenance;

use std::sync::Arc;

use crate::db::mapping::{new_id, now};
use crate::db::query_builder::path_upper_bound;
use crate::db::DatabasePool;
use crate::error::{KeystoneError, Result};
use crate::models::{LibraryFolder, WatchedFolder};

pub struct LibraryRepository {
    db: Arc<DatabasePool>,
}

impl LibraryRepository {
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    pub fn list_watched(&self) -> Result<Vec<WatchedFolder>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, is_active, added_at FROM watched_folders ORDER BY path ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(WatchedFolder {
                id: r.get(0)?,
                path: r.get(1)?,
                is_active: r.get(2)?,
                track_count: 0,
                added_at: r.get(3)?,
            })
        })?;
        let mut out: Vec<WatchedFolder> = rows.collect::<rusqlite::Result<_>>()?;

        // One count per root rather than a correlated subquery: the upper bound
        // of the path range has to be computed in Rust, and there are a handful
        // of roots, not a handful of thousands.
        for folder in &mut out {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tracks WHERE path >= ?1 AND path < ?2 \
                 AND index_status != 'missing'",
                rusqlite::params![&folder.path, path_upper_bound(&folder.path)],
                |r| r.get(0),
            )?;
            folder.track_count = n as usize;
        }
        Ok(out)
    }

    /// Upserts on path, so adding a folder that is already watched promotes it
    /// rather than failing on the UNIQUE constraint. Every migration path here
    /// is remove-then-re-add, so this has to be idempotent.
    pub fn add_watched(&self, path: &str) -> Result<WatchedFolder> {
        let conn = self.db.get()?;
        conn.execute(
            "INSERT INTO watched_folders (id, path, is_active, added_at) VALUES (?1, ?2, 1, ?3) \
             ON CONFLICT(path) DO UPDATE SET is_active = 1",
            rusqlite::params![new_id(), path, now()],
        )?;
        drop(conn);
        self.list_watched()?
            .into_iter()
            .find(|w| w.path == path)
            .ok_or_else(|| KeystoneError::NotFound(format!("watched folder {path}")))
    }

    /// Cascade-deletes the tracks under this root. The caller must call
    /// `Indexer::exclude` **before** this: a scan started earlier can still
    /// have hundreds of parsed rows in flight, and those writes would land
    /// after the delete and resurrect the folder.
    pub fn remove_watched(&self, path: &str) -> Result<usize> {
        let mut conn = self.db.get()?;
        let tx = conn.transaction()?;
        let removed = tx.execute(
            "DELETE FROM tracks WHERE path >= ?1 AND path < ?2",
            rusqlite::params![path, path_upper_bound(path)],
        )?;
        tx.execute("DELETE FROM watched_folders WHERE path = ?", [path])?;
        tx.commit()?;
        Ok(removed)
    }

    /// Distinct parent directories of live tracks, for the add-tracks flow.
    ///
    /// The parent is cut from `path` using the stored `filename` length, which
    /// is separator-agnostic and needs no path parsing in SQL — the filename is
    /// always the last component of its own path.
    pub fn folders_with_counts(&self) -> Result<Vec<LibraryFolder>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT substr(path, 1, length(path) - length(filename) - 1) AS folder, COUNT(*) \
             FROM tracks WHERE index_status != 'missing' \
             GROUP BY folder ORDER BY folder ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            let path: String = r.get(0)?;
            let name = path
                .rsplit(['/', '\\'])
                .find(|s| !s.is_empty())
                .unwrap_or(&path)
                .to_string();
            Ok(LibraryFolder {
                path,
                name,
                track_count: r.get::<_, i64>(1)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
