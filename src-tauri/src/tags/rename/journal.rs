//! The rename journal.
//!
//! **Renaming is the only irreversible operation in the app**, and one setting
//! runs it automatically after processing. The row exists before `fs::rename`
//! and is cleared after the database UPDATE, so a crash in between is
//! recoverable rather than a row pointing at a path that no longer exists.
//!
//! The SQL is here rather than in `db/` for one reason, recorded so it gets
//! fixed rather than copied: the scaffold put the rename engine outside the
//! repository layer, and `file_ops` has no repository of its own yet. It wants
//! moving to `db/file_ops_repo.rs` behind these same five functions.

use std::path::Path;
use std::sync::Arc;

use rusqlite::params;

use super::paths;
use crate::db::TrackRepository;
use crate::error::Result;

pub struct JournalRow {
    pub id: String,
    pub track_id: String,
    pub from_path: String,
    pub to_path: String,
    pub state: String,
}

/// The file has not moved yet.
pub const INTENDED: &str = "intended";
/// The file has moved; the row still owns the database update.
pub const RENAMED: &str = "renamed";

pub fn open(repo: &Arc<TrackRepository>, track_id: &str, from: &str, to: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    repo.conn()?.execute(
        "INSERT INTO file_ops (id, track_id, op, from_path, to_path, state, created_at) \
         VALUES (?1, ?2, 'rename', ?3, ?4, ?5, ?6)",
        params![&id, track_id, from, to, INTENDED, now()],
    )?;
    Ok(id)
}

pub fn mark_renamed(repo: &Arc<TrackRepository>, id: &str) -> Result<()> {
    repo.conn()?
        .execute("UPDATE file_ops SET state = ?1 WHERE id = ?2", params![RENAMED, id])?;
    Ok(())
}

/// Clears the row. The table is empty whenever nothing is in flight, which is
/// what makes startup recovery a single indexed read of almost nothing.
pub fn close(repo: &Arc<TrackRepository>, id: &str) -> Result<()> {
    repo.conn()?
        .execute("DELETE FROM file_ops WHERE id = ?1", [id])?;
    Ok(())
}

pub fn pending(repo: &Arc<TrackRepository>) -> Result<Vec<JournalRow>> {
    let conn = repo.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, track_id, from_path, to_path, state FROM file_ops \
         WHERE op = 'rename' ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(JournalRow {
            id: row.get(0)?,
            track_id: row.get(1)?,
            from_path: row.get(2)?,
            to_path: row.get(3)?,
            state: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Replays rows left behind by a crash. Called on startup, before anything
/// reads a path.
///
/// Every branch decides from the filesystem, never from the stored state alone:
/// the state says what Keystone *intended*, and the whole reason this function
/// exists is that the two can disagree.
pub fn recover(repo: &Arc<TrackRepository>) -> Result<usize> {
    let mut recovered = 0;
    for row in pending(repo)? {
        if recover_one(repo, &row)? {
            recovered += 1;
        }
        close(repo, &row.id)?;
    }
    Ok(recovered)
}

fn recover_one(repo: &Arc<TrackRepository>, row: &JournalRow) -> Result<bool> {
    let from = Path::new(&row.from_path);
    let to = Path::new(&row.to_path);

    // The move landed. Whether the state says `intended` or `renamed`, the
    // database row is the thing that is behind.
    if to.exists() && !from.exists() {
        finish(repo, row)?;
        return Ok(true);
    }

    // Nothing moved. The row already points at `from`, so there is nothing to
    // repair — unless this was a case-only rename, which parks the file under a
    // staging name between its two steps.
    if from.exists() && !to.exists() {
        return Ok(false);
    }

    if !from.exists() && !to.exists() {
        if let Some(staged) = find_staged(to) {
            std::fs::rename(&staged, to)?;
            finish(repo, row)?;
            return Ok(true);
        }
        // Both gone and no staging file: the reconciler will mark the track
        // missing. Guessing here would invent a path.
        tracing::warn!(
            "rename journal: neither {} nor {} exists",
            row.from_path,
            row.to_path
        );
        return Ok(false);
    }

    // Both exist. Two files, one of which Keystone may have half-written; the
    // only safe move is to leave them alone and keep the row pointing at the
    // name it already had.
    tracing::warn!(
        "rename journal: both {} and {} exist, leaving both alone",
        row.from_path,
        row.to_path
    );
    Ok(false)
}

fn finish(repo: &Arc<TrackRepository>, row: &JournalRow) -> Result<()> {
    let original = Path::new(&row.from_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    // `update_path` COALESCEs `original_filename`, so handing it the name this
    // rename started from is right whether or not the track was renamed before.
    repo.update_path(&row.track_id, &row.to_path, original.as_deref(), None)
}

/// Looks for the file parked by the first half of a case-only rename.
///
/// The staging name is `.keystone-<pid>-<stamp>-<target name>`, and the match is
/// on the normalised target name so an NFD directory entry still matches an NFC
/// path from the database.
fn find_staged(to: &Path) -> Option<std::path::PathBuf> {
    let dir = to.parent()?;
    let wanted = paths::nfc(&to.file_name()?.to_string_lossy());
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = paths::nfc(&entry.file_name().to_string_lossy());
        if name.starts_with(".keystone-") && name.ends_with(&wanted) {
            return Some(entry.path());
        }
    }
    None
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
