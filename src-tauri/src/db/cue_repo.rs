//! Cue points and the beat grid.
//!
//! The whole file turns on one distinction: a `detected` cue belongs to the
//! engine and a `user` cue belongs to the DJ. Re-analysis may replace the first
//! and must never touch the second — a marker somebody dragged into place is
//! the work the app exists to protect.

use std::collections::HashSet;
use std::sync::Arc;

use rusqlite::params_from_iter;

use crate::db::mapping::{cue_source, cue_source_str, new_id, now, placeholders, IN_CHUNK};
use crate::db::DatabasePool;
use crate::error::{KeystoneError, Result};
use crate::models::{BeatgridMarker, CueEdit, CueKind, CuePoint, CueSource, DetectedCue};

const CUE_COLUMNS: &str = "id, track_id, slot, kind, position_ms, end_ms, bar_index, \
     phrase_level, label, color, confidence, source, created_at";

pub struct CueRepository {
    db: Arc<DatabasePool>,
}

impl CueRepository {
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    pub fn list_for_track(&self, track_id: &str) -> Result<Vec<CuePoint>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {CUE_COLUMNS} FROM cue_points WHERE track_id = ? ORDER BY slot ASC"
        ))?;
        let rows = stmt.query_map([track_id], row_to_cue)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Cue counts for the table's CUE POINTS column, in one query rather than
    /// one per visible row.
    pub fn counts_for_tracks(&self, track_ids: &[String]) -> Result<Vec<(String, usize)>> {
        let conn = self.db.get()?;
        let mut out = Vec::with_capacity(track_ids.len());
        for chunk in track_ids.chunks(IN_CHUNK) {
            let sql = format!(
                "SELECT track_id, COUNT(*) FROM cue_points WHERE track_id IN ({}) GROUP BY track_id",
                placeholders(chunk.len())
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(chunk.iter()), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// Replaces the detected set.
    ///
    /// A user cue holding a slot means the detected cue for that slot is
    /// **dropped, not relocated**: moving it to a free slot would renumber the
    /// DJ's `CUE 3` behind their back, and the slot number is what the hot-cue
    /// pads are mapped to.
    pub fn replace_detected(&self, track_id: &str, cues: &[DetectedCue]) -> Result<usize> {
        let mut conn = self.db.get()?;
        let tx = conn.transaction()?;
        let held: HashSet<u8> = {
            let mut stmt =
                tx.prepare("SELECT slot FROM cue_points WHERE track_id = ? AND source = 'user'")?;
            let rows = stmt.query_map([track_id], |r| r.get::<_, u8>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        tx.execute(
            "DELETE FROM cue_points WHERE track_id = ? AND source = 'detected'",
            [track_id],
        )?;
        let mut inserted = 0usize;
        {
            let mut stmt = tx.prepare_cached(INSERT_CUE_SQL)?;
            let stamp = now();
            for cue in cues.iter().filter(|c| !held.contains(&c.slot)) {
                inserted += stmt.execute(rusqlite::params![
                    new_id(),
                    track_id,
                    cue.slot,
                    cue.kind.as_str(),
                    cue.position_ms,
                    cue.end_ms,
                    cue.bar_index,
                    cue.phrase_level,
                    cue.label,
                    Option::<String>::None,
                    cue.confidence,
                    cue_source_str(CueSource::Detected),
                    stamp,
                ])?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Adding or dragging a marker. Writes `source = 'user'`, which is what
    /// protects it from the next analysis pass.
    pub fn upsert_user(&self, edit: &CueEdit) -> Result<CuePoint> {
        let conn = self.db.get()?;
        conn.execute(
            "INSERT INTO cue_points (id, track_id, slot, kind, position_ms, end_ms, bar_index, \
             phrase_level, label, color, confidence, source, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7, ?8, NULL, 'user', ?9) \
             ON CONFLICT(track_id, slot) DO UPDATE SET \
               kind        = excluded.kind, \
               position_ms = excluded.position_ms, \
               end_ms      = excluded.end_ms, \
               label       = COALESCE(excluded.label, cue_points.label), \
               color       = COALESCE(excluded.color, cue_points.color), \
               bar_index   = NULL, \
               phrase_level = NULL, \
               confidence  = NULL, \
               source      = 'user'",
            rusqlite::params![
                new_id(),
                edit.track_id,
                edit.slot,
                edit.kind.unwrap_or(CueKind::User).as_str(),
                edit.position_ms,
                edit.end_ms,
                edit.label,
                edit.color,
                now(),
            ],
        )?;
        self.get_by_slot(&edit.track_id, edit.slot)
    }

    pub fn move_cue(&self, cue_id: &str, position_ms: f64) -> Result<CuePoint> {
        let conn = self.db.get()?;
        // Dragging makes it theirs. The bar index and phrase level came from the
        // detector's grid and no longer describe this position, so they go.
        let changed = conn.execute(
            "UPDATE cue_points SET position_ms = ?1, source = 'user', bar_index = NULL, \
             phrase_level = NULL, confidence = NULL WHERE id = ?2",
            rusqlite::params![position_ms, cue_id],
        )?;
        if changed == 0 {
            return Err(KeystoneError::NotFound(format!("cue point {cue_id}")));
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT {CUE_COLUMNS} FROM cue_points WHERE id = ?"
        ))?;
        Ok(stmt.query_row([cue_id], row_to_cue)?)
    }

    pub fn delete(&self, cue_id: &str) -> Result<()> {
        self.db
            .get()?
            .execute("DELETE FROM cue_points WHERE id = ?", [cue_id])?;
        Ok(())
    }

    /// `include_user` is the difference between the context menu's Remove Cue
    /// Points and a re-analysis clearing its own work.
    pub fn delete_for_track(&self, track_id: &str, include_user: bool) -> Result<usize> {
        let conn = self.db.get()?;
        let n = if include_user {
            conn.execute("DELETE FROM cue_points WHERE track_id = ?", [track_id])?
        } else {
            conn.execute(
                "DELETE FROM cue_points WHERE track_id = ? AND source = 'detected'",
                [track_id],
            )?
        };
        Ok(n)
    }

    pub fn beatgrid(&self, track_id: &str) -> Result<Vec<BeatgridMarker>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, track_id, position_ms, bpm, beats_per_bar, beat_in_bar \
             FROM beatgrid WHERE track_id = ? ORDER BY position_ms ASC",
        )?;
        let rows = stmt.query_map([track_id], |r| {
            Ok(BeatgridMarker {
                id: r.get(0)?,
                track_id: r.get(1)?,
                position_ms: r.get(2)?,
                bpm: r.get(3)?,
                beats_per_bar: r.get(4)?,
                beat_in_bar: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Two markers for a constant-tempo track, not one per beat. rekordbox's
    /// own exports write one per beat, which is why they are enormous; what
    /// matters is that rekordbox's grid agrees with ours, or every imported cue
    /// drifts.
    pub fn replace_beatgrid(&self, track_id: &str, markers: &[BeatgridMarker]) -> Result<()> {
        let mut conn = self.db.get()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM beatgrid WHERE track_id = ?", [track_id])?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO beatgrid (id, track_id, position_ms, bpm, beats_per_bar, beat_in_bar) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for m in markers {
                stmt.execute(rusqlite::params![
                    new_id(),
                    track_id,
                    m.position_ms,
                    m.bpm,
                    m.beats_per_bar,
                    m.beat_in_bar,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn get_by_slot(&self, track_id: &str, slot: u8) -> Result<CuePoint> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {CUE_COLUMNS} FROM cue_points WHERE track_id = ?1 AND slot = ?2"
        ))?;
        Ok(stmt.query_row(rusqlite::params![track_id, slot], row_to_cue)?)
    }
}

const INSERT_CUE_SQL: &str = "\
INSERT INTO cue_points (id, track_id, slot, kind, position_ms, end_ms, bar_index, \
  phrase_level, label, color, confidence, source, created_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

fn row_to_cue(r: &rusqlite::Row<'_>) -> rusqlite::Result<CuePoint> {
    Ok(CuePoint {
        id: r.get(0)?,
        track_id: r.get(1)?,
        slot: r.get(2)?,
        // An unrecognised kind becomes `User`: that keeps the marker and keeps
        // it out of the detector's way, which is the recoverable direction.
        kind: CueKind::parse(&r.get::<_, String>(3)?).unwrap_or(CueKind::User),
        position_ms: r.get(4)?,
        end_ms: r.get(5)?,
        bar_index: r.get(6)?,
        phrase_level: r.get(7)?,
        label: r.get(8)?,
        color: r.get(9)?,
        confidence: r.get(10)?,
        source: cue_source(&r.get::<_, String>(11)?),
        created_at: r.get(12)?,
    })
}

#[cfg(test)]
#[path = "cue_repo_tests.rs"]
mod tests;
