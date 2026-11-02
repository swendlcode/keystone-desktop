use std::sync::Arc;

use crate::db::mapping::{new_id, now, placeholders};
use crate::db::DatabasePool;
use crate::error::Result;
use crate::models::{AnalysisResult, DetectedRegion, KeyMode, RegionKind, TrackRegion};

/// The analysis cache plus the region read model.
///
/// Keying results by content hash is the highest-value piece of the data model:
/// a moved file, a file Keystone itself renamed, and a duplicate all hit it with
/// zero DSP, which is what makes a re-scan nearly free and the rename feature
/// free instead of a re-analysis storm.
pub struct AnalysisRepository {
    db: Arc<DatabasePool>,
}

impl AnalysisRepository {
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// Invalidation is on `content_hash` + `analysis_version`, never on mtime.
    /// A tag edit changes mtime but not audio and must not cost a re-analysis.
    ///
    /// `energy_model_version` is stored but deliberately not part of the lookup
    /// key: the primary key is `(content_hash, analysis_version)`, and an energy
    /// model bump is handled by bumping the analysis version with it, so there
    /// is one invalidation story rather than two that can disagree.
    pub fn lookup(
        &self,
        content_hash: &str,
        analysis_version: i32,
    ) -> Result<Option<AnalysisResult>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT payload FROM analysis_cache WHERE content_hash = ?1 AND analysis_version = ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![content_hash, analysis_version])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let payload: String = row.get(0)?;
        // A payload written by a build whose structs have since changed is a
        // miss, not an error: re-analysing one track is cheaper than refusing
        // to open the library.
        Ok(serde_json::from_str(&payload).ok())
    }

    pub fn store(&self, result: &AnalysisResult) -> Result<()> {
        let payload = serde_json::to_string(result)?;
        self.db.get()?.execute(
            "INSERT INTO analysis_cache \
             (content_hash, analysis_version, energy_model_version, payload, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(content_hash, analysis_version) DO UPDATE SET \
               energy_model_version = excluded.energy_model_version, \
               payload = excluded.payload, \
               created_at = excluded.created_at",
            rusqlite::params![
                result.content_hash,
                result.analysis_version,
                result.energy_model_version,
                payload,
                now(),
            ],
        )?;
        Ok(())
    }

    /// Drops cache rows for versions nothing can read any more.
    pub fn prune(&self, keep_analysis_versions: &[i32]) -> Result<usize> {
        if keep_analysis_versions.is_empty() {
            // An empty keep list means "keep nothing", which would silently
            // throw the whole cache away. Refuse: a caller that means it can
            // pass the versions it wants gone.
            return Ok(0);
        }
        let sql = format!(
            "DELETE FROM analysis_cache WHERE analysis_version NOT IN ({})",
            placeholders(keep_analysis_versions.len())
        );
        let conn = self.db.get()?;
        Ok(conn.execute(
            &sql,
            rusqlite::params_from_iter(keep_analysis_versions.iter()),
        )?)
    }

    /// Cache rows no live track points at any more. Separate from [`Self::prune`]
    /// because a version bump and an emptied library are different events.
    pub fn prune_orphans(&self) -> Result<usize> {
        let conn = self.db.get()?;
        Ok(conn.execute(
            "DELETE FROM analysis_cache WHERE content_hash NOT IN \
             (SELECT content_hash FROM tracks WHERE content_hash IS NOT NULL)",
            [],
        )?)
    }

    pub fn regions(&self, track_id: &str) -> Result<Vec<TrackRegion>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, track_id, start_ms, end_ms, bar_index, bars, kind, energy, \
             key_root, key_mode FROM track_regions WHERE track_id = ? ORDER BY start_ms ASC",
        )?;
        let rows = stmt.query_map([track_id], |r| {
            Ok(TrackRegion {
                id: r.get(0)?,
                track_id: r.get(1)?,
                start_ms: r.get(2)?,
                end_ms: r.get(3)?,
                bar_index: r.get(4)?,
                bars: r.get(5)?,
                kind: r
                    .get::<_, Option<String>>(6)?
                    .as_deref()
                    .and_then(RegionKind::parse)
                    .unwrap_or(RegionKind::Unknown),
                energy: r.get(7)?,
                key_root: r.get(8)?,
                key_mode: r
                    .get::<_, Option<String>>(9)?
                    .as_deref()
                    .and_then(KeyMode::parse),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn replace_regions(&self, track_id: &str, regions: &[DetectedRegion]) -> Result<()> {
        let mut conn = self.db.get()?;
        let tx = crate::db::write_tx(&mut conn)?;
        tx.execute("DELETE FROM track_regions WHERE track_id = ?", [track_id])?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO track_regions \
                 (id, track_id, start_ms, end_ms, bar_index, bars, kind, energy, key_root, key_mode) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for region in regions {
                stmt.execute(rusqlite::params![
                    new_id(),
                    track_id,
                    region.start_ms,
                    region.end_ms,
                    region.bar_index,
                    region.bars,
                    region.kind.as_str(),
                    region.energy,
                    region.key_root,
                    region.key_mode.map(KeyMode::as_str),
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "analysis_repo_tests.rs"]
mod tests;
