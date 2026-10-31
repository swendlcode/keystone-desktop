//! Tracks: the table's read paths and the batched writer behind the walk.
//!
//! Split because this is the busiest repository in the app and one file
//! outgrows the 300-line cap: reads here, the batched walk and analysis writes
//! in [`writes`], the per-field user edits in [`edits`], the two thirty-column
//! statements in [`sql`], and the row projection in [`rows`].

mod edits;
mod journal;
mod rows;
mod sql;
mod writes;

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::params_from_iter;

use crate::db::mapping::{placeholders, IN_CHUNK};
use crate::db::query_builder::{self as qb, TRACK_COLUMNS};
use crate::db::{DatabasePool, PooledConn};
use crate::error::Result;
use crate::models::{Track, TrackFilters, TrackPage, TrackQuery, WheelCounts};

pub use journal::FileOp;
pub use rows::row_to_track;

/// How many parsed rows one transaction commits. Per-file commits spend the
/// whole budget in fsync; much larger batches hold the single writer long
/// enough that readers start hitting the busy timeout.
pub const UPSERT_BATCH_SIZE: usize = 100;

pub struct TrackRepository {
    db: Arc<DatabasePool>,
}

impl TrackRepository {
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    // --- reads ------------------------------------------------------------

    pub fn get_by_id(&self, id: &str) -> Result<Option<Track>> {
        self.one(&format!("{TRACK_COLUMNS} FROM tracks WHERE id = ?"), id)
    }

    pub fn get_by_path(&self, path: &str) -> Result<Option<Track>> {
        self.one(&format!("{TRACK_COLUMNS} FROM tracks WHERE path = ?"), path)
    }

    fn one(&self, tail: &str, bind: &str) -> Result<Option<Track>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(&format!("SELECT {tail}"))?;
        let mut rows = stmt.query([bind])?;
        match rows.next()? {
            Some(r) => Ok(Some(row_to_track(r)?)),
            None => Ok(None),
        }
    }

    /// Chunked under SQLite's bound-variable limit, because the table hands
    /// this whole visible selection at once.
    pub fn get_many(&self, ids: &[String]) -> Result<Vec<Track>> {
        let conn = self.db.get()?;
        let mut out = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(IN_CHUNK) {
            let sql = format!(
                "SELECT {TRACK_COLUMNS} FROM tracks WHERE id IN ({})",
                placeholders(chunk.len())
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map(params_from_iter(chunk.iter()), |r| row_to_track(r))?;
            for t in mapped {
                out.push(t?);
            }
        }
        Ok(out)
    }

    /// The table's one read path. Returns the page and the total in one call
    /// because the header shows `412 TRACKS` beside the rows.
    pub fn search(&self, query: &TrackQuery) -> Result<TrackPage> {
        let (where_sql, params) = qb::build_where(&query.filters);
        let order = qb::build_order(query.sort, query.direction);
        let conn = self.db.get()?;

        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE {where_sql} {order} LIMIT ? OFFSET ?"
        );
        let mut bound = params.clone();
        bound.push(rusqlite::types::Value::Integer(query.limit as i64));
        bound.push(rusqlite::types::Value::Integer(query.offset as i64));

        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map(params_from_iter(bound.iter()), |r| row_to_track(r))?;
        let mut tracks = Vec::with_capacity(query.limit.min(1024));
        for t in mapped {
            tracks.push(t?);
        }

        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM tracks WHERE {where_sql}"),
            params_from_iter(params.iter()),
            |r| r.get(0),
        )?;

        Ok(TrackPage {
            tracks,
            total: total as usize,
        })
    }

    pub fn count(&self, filters: &TrackFilters) -> Result<usize> {
        let (where_sql, params) = qb::build_where(filters);
        let conn = self.db.get()?;
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM tracks WHERE {where_sql}"),
            params_from_iter(params.iter()),
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// The genre vocabulary the library filter offers.
    ///
    /// Read out of the library rather than from a curated list, for the same
    /// reason the wheel counts are computed rather than assumed: a genre the
    /// user owns no tracks in is an option that filters to nothing, and one
    /// they do own that a hardcoded list omits is unreachable. Missing rows are
    /// excluded so a folder that went away stops offering its genres.
    pub fn distinct_genres(&self) -> Result<Vec<String>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT genre FROM tracks \
             WHERE genre IS NOT NULL AND genre <> '' AND index_status != 'missing' \
             ORDER BY genre COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// One count per wheel segment, so a segment with nothing behind it reads
    /// as empty instead of filtering to zero rows.
    ///
    /// The incoming wheel selection is deliberately dropped: a segment's count
    /// must not depend on which segment is currently lit, or clicking one
    /// empties the other twenty-three.
    pub fn wheel_counts(&self, filters: &TrackFilters) -> Result<WheelCounts> {
        let mut unselected = filters.clone();
        unselected.camelot = None;
        let (where_sql, params) = qb::build_where(&unselected);

        let conn = self.db.get()?;
        let sql = format!(
            "SELECT camelot, COUNT(*) FROM tracks WHERE {where_sql} GROUP BY camelot"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?))
        })?;

        let mut found: HashMap<String, usize> = HashMap::new();
        let mut unkeyed = 0usize;
        for row in rows {
            let (label, n) = row?;
            match label {
                Some(l) => {
                    *found.entry(l).or_default() += n as usize;
                }
                None => unkeyed += n as usize,
            }
        }

        // Emitted in wheel order and always 24 long, so the frontend can index
        // segments positionally instead of searching the list.
        let mut counts = Vec::with_capacity(24);
        for letter in ['A', 'B'] {
            for number in 1..=12u8 {
                let label = format!("{number}{letter}");
                let n = found.get(&label).copied().unwrap_or(0);
                counts.push((label, n));
            }
        }
        Ok(WheelCounts { counts, unkeyed })
    }

    /// Tracks whose analysis is absent or stale. Drives the queue's
    /// resume-across-restarts behaviour, so it must be cheap and indexed — the
    /// predicate is spelled to match `idx_tracks_analysis_ver`.
    pub fn needing_analysis(&self, analysis_version: i32, limit: usize) -> Result<Vec<Track>> {
        let conn = self.db.get()?;
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks \
             WHERE analysis_version < ? AND {} \
             ORDER BY date_added ASC, id ASC LIMIT ?",
            qb::LIVE_PREDICATE
        );
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map(
            rusqlite::params![analysis_version, limit as i64],
            |r| row_to_track(r),
        )?;
        let mut out = Vec::new();
        for t in mapped {
            out.push(t?);
        }
        Ok(out)
    }

    /// Chunked so a 20,000-track root does not build one enormous Vec and one
    /// enormous IN list.
    pub fn existing_paths(&self, root: &str, offset: usize, limit: usize) -> Result<Vec<String>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT path FROM tracks WHERE path >= ?1 AND path < ?2 \
             ORDER BY path ASC LIMIT ?3 OFFSET ?4",
        )?;
        let mapped = stmt.query_map(
            rusqlite::params![root, qb::path_upper_bound(root), limit as i64, offset as i64],
            |r| r.get::<_, String>(0),
        )?;
        let mut out = Vec::new();
        for p in mapped {
            out.push(p?);
        }
        Ok(out)
    }

    /// Of `(path, mtime)` candidates, the ones the walk may skip.
    ///
    /// There is no `mtime` column, so the comparison is against `updated_at`:
    /// any write to the file pushes its mtime past the moment this row was last
    /// written, and the tag writer restores mtime deliberately after a write it
    /// has already recorded. Erring towards re-reading is the safe direction —
    /// the cost of a wrong "stale" is one header probe, the cost of a wrong
    /// "skip" is a row that never catches up with the file.
    pub fn unchanged_since(&self, candidates: &[(String, i64)]) -> Result<Vec<String>> {
        let conn = self.db.get()?;
        let mut skippable = Vec::new();
        for chunk in candidates.chunks(IN_CHUNK) {
            let sql = format!(
                "SELECT path, updated_at FROM tracks \
                 WHERE index_status = 'indexed' AND path IN ({})",
                placeholders(chunk.len())
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(chunk.iter().map(|(p, _)| p)), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?;
            let known: HashMap<String, i64> = rows.collect::<rusqlite::Result<_>>()?;
            for (path, mtime) in chunk {
                if known.get(path).is_some_and(|updated| *mtime <= *updated) {
                    skippable.push(path.clone());
                }
            }
        }
        Ok(skippable)
    }

    /// Exposed so the dedicated writer task can run several statements inside
    /// one transaction without going back through the pool per statement.
    pub(crate) fn conn(&self) -> Result<PooledConn> {
        self.db.get()
    }
}

#[cfg(test)]
mod tests {
    mod common;
    mod reads;
    mod search;
    mod writes;
}
