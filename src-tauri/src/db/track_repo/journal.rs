//! The rename journal (`file_ops`).
//!
//! A rename is the only irreversible thing the app does, so the intent is
//! written here *before* `fs::rename` and the row is cleared *after* the
//! `tracks` UPDATE. A crash between the two leaves exactly one open row, which
//! is what [`crate::core::renamer::recover_journal`] replays at launch.
//!
//! The states are the three points the window can be caught at:
//!
//! - `intended` — the row exists, the file has not moved yet.
//! - `renamed` — the file has moved, the `tracks` row still points at the old
//!   path.
//! - `done` — both sides are consistent; the row is only still here because the
//!   process died before the DELETE.

use rusqlite::params;

use super::TrackRepository;
use crate::db::mapping::now;
use crate::error::Result;

/// One journal row. Never crosses the IPC boundary: recovery is a backend
/// decision and the frontend sees its outcome in the track row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOp {
    pub id: String,
    pub track_id: String,
    /// Only `rename` today. Kept as the stored string so an unknown op from a
    /// newer build is skipped with a warning instead of being misread as one.
    pub op: String,
    pub from_path: String,
    pub to_path: String,
    pub state: String,
    pub created_at: i64,
}

impl TrackRepository {
    /// Every row that is not `done`, oldest first.
    ///
    /// Ordered because two renames can touch the same file in sequence
    /// (`A -> B`, then `B -> C`), and replaying those out of order would decide
    /// the first one against a filesystem the second one had already changed.
    pub fn open_file_ops(&self) -> Result<Vec<FileOp>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, track_id, op, from_path, to_path, state, created_at \
             FROM file_ops WHERE state != 'done' ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(FileOp {
                id: r.get(0)?,
                track_id: r.get(1)?,
                op: r.get(2)?,
                from_path: r.get(3)?,
                to_path: r.get(4)?,
                state: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Writes the intent. Returns the row id, which the caller clears once the
    /// track row agrees with the filesystem again.
    pub fn record_file_op(&self, track_id: &str, from_path: &str, to_path: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn()?.execute(
            "INSERT INTO file_ops (id, track_id, op, from_path, to_path, state, created_at) \
             VALUES (?1, ?2, 'rename', ?3, ?4, 'intended', ?5)",
            params![id, track_id, from_path, to_path, now()],
        )?;
        Ok(id)
    }

    /// The file has moved; the `tracks` row has not caught up yet. Recorded so a
    /// crash in that gap is distinguishable from a crash before the move.
    pub fn mark_file_op_renamed(&self, id: &str) -> Result<()> {
        self.conn()?
            .execute("UPDATE file_ops SET state = 'renamed' WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Both sides agree: the row has nothing left to say.
    pub fn clear_file_op(&self, id: &str) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM file_ops WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Drops the `done` rows a crash left behind. They carry no decision, so
    /// recovery deletes them rather than replaying them.
    pub fn purge_finished_file_ops(&self) -> Result<usize> {
        Ok(self
            .conn()?
            .execute("DELETE FROM file_ops WHERE state = 'done'", [])?)
    }
}
