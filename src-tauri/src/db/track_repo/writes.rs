//! The write half of [`super::TrackRepository`]. Named parameters throughout:
//! these statements touch thirty columns and a positional list is one insertion
//! away from writing energy into the rating column.

use rusqlite::named_params;

use super::sql::{tag_column, APPLY_ANALYSIS_SQL, UPSERT_SQL};
use super::{TrackRepository, UPSERT_BATCH_SIZE};
use crate::analysis::tempo::fold_into_range;
use crate::db::mapping::{index_status_str, now, placeholders, IN_CHUNK};
use crate::error::Result;
use crate::models::{AnalysisResult, IndexStatus, KeyMode, TagField, TrackUpsert};

impl TrackRepository {
    /// The batched writer path. Conflicts on `path` and preserves the existing
    /// id, so a row keeps its identity across re-scans.
    pub fn upsert_batch(&self, rows: &[TrackUpsert]) -> Result<usize> {
        let mut conn = self.conn()?;
        let mut written = 0usize;
        for batch in rows.chunks(UPSERT_BATCH_SIZE) {
            let tx = crate::db::write_tx(&mut conn)?;
            {
                let mut stmt = tx.prepare_cached(UPSERT_SQL)?;
                for row in batch {
                    written += bind_upsert(&mut stmt, row)?;
                }
            }
            tx.commit()?;
        }
        Ok(written)
    }

    /// Writes one analysis result onto a row.
    ///
    /// Takes the fold range rather than reading settings: the repository has no
    /// business knowing about `settings.json`, and the caller already holds the
    /// lock. `tempo_raw` is stored unfolded so a later range change is a
    /// `refold_tempo_display` and never a re-analysis.
    pub fn apply_analysis(
        &self,
        track_id: &str,
        result: &AnalysisResult,
        tempo_range: (f64, f64),
    ) -> Result<()> {
        let (lo, hi) = tempo_range;
        let (display, shift) = fold_into_range(result.tempo.raw_bpm, lo, hi);
        let key = &result.key;
        let conn = self.conn()?;
        conn.execute(
            APPLY_ANALYSIS_SQL,
            named_params! {
                ":duration_ms": result.duration_ms,
                ":tempo_raw": result.tempo.raw_bpm,
                ":tempo_display": display,
                ":octave_shift": shift,
                ":tempo_confidence": result.tempo.confidence,
                ":residual_ms": result.tempo.residual_ms,
                ":variable": result.tempo.variable,
                ":key_root": key.root,
                ":key_mode": key.mode.as_str(),
                ":camelot": key.camelot,
                ":key_confidence": key.confidence,
                ":alt_root": key.alt_root,
                ":alt_mode": key.alt_mode.map(KeyMode::as_str),
                ":mode_ambiguous": key.mode_ambiguous,
                ":tuning_cents": key.tuning_cents,
                ":energy": result.energy.energy,
                ":energy_raw": result.energy.raw,
                ":peak_db": result.levels.peak_db,
                ":true_peak_db": result.levels.true_peak_db,
                ":lufs": result.levels.lufs_integrated,
                ":clipped": result.levels.clipped_peaks,
                ":analysis_version": result.analysis_version,
                ":energy_model_version": result.energy_model_version,
                ":now": now(),
                ":id": track_id,
            },
        )?;
        Ok(())
    }

    pub fn set_index_status(
        &self,
        track_id: &str,
        status: IndexStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET index_status = ?1, error_message = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![index_status_str(status), error, now(), track_id],
        )?;
        Ok(())
    }

    /// Re-folds `tempo_display` from `tempo_raw` for every row. This is what
    /// makes the Tempo range setting free: no file is read and no DSP runs.
    pub fn refold_tempo_display(&self, lo: f64, hi: f64) -> Result<usize> {
        let mut conn = self.conn()?;
        let raws: Vec<(String, f64)> = {
            let mut stmt =
                conn.prepare("SELECT id, tempo_raw FROM tracks WHERE tempo_raw IS NOT NULL")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };

        let tx = crate::db::write_tx(&mut conn)?;
        let mut changed = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE tracks SET tempo_display = ?1, tempo_octave_shift = ?2 WHERE id = ?3",
            )?;
            for (id, raw) in &raws {
                let (display, shift) = fold_into_range(*raw, lo, hi);
                changed += stmt.execute(rusqlite::params![display, shift, id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Touches `last_seen_at`, and restores a row the last pass marked missing.
    ///
    /// The restore is here rather than in a second statement because re-mounting
    /// a drive has to heal itself: a user should not have to find a re-scan
    /// button to undo a state they did not create.
    pub fn touch_last_seen_by_paths(&self, paths: &[String]) -> Result<usize> {
        let mut conn = self.conn()?;
        let stamp = now();
        let mut touched = 0usize;
        for chunk in paths.chunks(IN_CHUNK) {
            let sql = format!(
                "UPDATE tracks SET last_seen_at = ?, \
                 index_status = CASE WHEN index_status = 'missing' THEN 'indexed' \
                                     ELSE index_status END \
                 WHERE path IN ({})",
                placeholders(chunk.len())
            );
            let tx = crate::db::write_tx(&mut conn)?;
            {
                let mut stmt = tx.prepare(&sql)?;
                let mut bound: Vec<rusqlite::types::Value> =
                    vec![rusqlite::types::Value::Integer(stamp)];
                bound.extend(chunk.iter().map(|p| rusqlite::types::Value::Text(p.clone())));
                touched += stmt.execute(rusqlite::params_from_iter(bound.iter()))?;
            }
            tx.commit()?;
        }
        Ok(touched)
    }

    /// Marks exactly these paths missing.
    ///
    /// Explicit rather than a `last_seen_at < walk_started` pivot: these
    /// timestamps are whole seconds, so a row written in the same second as the
    /// pass that should have marked it is indistinguishable from one the pass
    /// just touched. Naming the paths is precise at any clock resolution.
    pub fn mark_missing_by_paths(&self, paths: &[String]) -> Result<usize> {
        let mut conn = self.conn()?;
        let stamp = now();
        let mut marked = 0usize;
        for chunk in paths.chunks(IN_CHUNK) {
            let sql = format!(
                "UPDATE tracks SET index_status = 'missing', updated_at = ? \
                 WHERE index_status != 'missing' AND path IN ({})",
                placeholders(chunk.len())
            );
            let tx = crate::db::write_tx(&mut conn)?;
            {
                let mut bound: Vec<rusqlite::types::Value> =
                    vec![rusqlite::types::Value::Integer(stamp)];
                bound.extend(chunk.iter().map(|p| rusqlite::types::Value::Text(p.clone())));
                marked += tx.execute(&sql, rusqlite::params_from_iter(bound.iter()))?;
            }
            tx.commit()?;
        }
        Ok(marked)
    }

    /// Global Replace. Returns the number of *rows* that would change, which is
    /// what the confirm step shows before a single file is touched.
    ///
    /// The fields are OR'd into one count rather than counted one at a time and
    /// summed: a track whose artist and title both contain the needle is one
    /// file being rewritten, and a confirm dialog that says two is a dialog the
    /// user cannot reconcile with the result.
    pub fn preview_replace(
        &self,
        fields: &[TagField],
        find: &str,
        track_ids: Option<&[String]>,
    ) -> Result<usize> {
        if find.is_empty() || fields.is_empty() {
            return Ok(0);
        }
        // `instr` is byte-exact, so no LIKE escaping and no accidental
        // case-insensitive match on a column the user is replacing literally.
        // The needle is `?1` in every branch of the OR, so the id placeholders
        // that follow number from 2 and the single bound value is reused.
        let matches = fields
            .iter()
            .map(|f| format!("instr({}, ?1) > 0", tag_column(*f)))
            .collect::<Vec<_>>()
            .join(" OR ");
        let conn = self.conn()?;
        match track_ids {
            None => {
                let n: i64 = conn.query_row(
                    &format!("SELECT COUNT(*) FROM tracks WHERE ({matches})"),
                    [find],
                    |r| r.get(0),
                )?;
                Ok(n as usize)
            }
            Some(ids) => {
                let mut total = 0i64;
                for chunk in ids.chunks(IN_CHUNK) {
                    let sql = format!(
                        "SELECT COUNT(*) FROM tracks WHERE ({matches}) AND id IN ({})",
                        placeholders(chunk.len())
                    );
                    let mut bound: Vec<rusqlite::types::Value> =
                        vec![rusqlite::types::Value::Text(find.to_string())];
                    bound.extend(chunk.iter().map(|i| rusqlite::types::Value::Text(i.clone())));
                    total += conn.query_row(
                        &sql,
                        rusqlite::params_from_iter(bound.iter()),
                        |r| r.get::<_, i64>(0),
                    )?;
                }
                Ok(total as usize)
            }
        }
    }
}

fn bind_upsert(stmt: &mut rusqlite::CachedStatement<'_>, row: &TrackUpsert) -> Result<usize> {
    let tags = row.tags.clone().unwrap_or_default();
    // `None` when tags were not read at all, so the COALESCE leaves a
    // previously-read flag alone instead of clearing it to 0.
    let cover: Option<bool> = row.tags.as_ref().map(|t| t.has_cover_art);
    let stamp = now();
    Ok(stmt.execute(named_params! {
        ":id": row.id,
        ":path": row.path,
        ":filename": row.filename,
        ":extension": row.extension,
        ":size_bytes": row.size_bytes,
        ":content_hash": row.content_hash,
        ":duration_ms": row.duration_ms,
        ":sample_rate": row.sample_rate,
        ":channels": row.channels,
        ":bit_depth": row.bit_depth,
        ":bitrate_kbps": row.bitrate_kbps,
        ":codec": row.codec,
        ":artist": tags.artist,
        ":title": tags.title,
        ":album": tags.album,
        ":genre": tags.genre,
        ":grouping": tags.grouping,
        ":comment": tags.comment,
        ":year": tags.year,
        ":tag_bpm": tags.bpm,
        ":tag_key": tags.initial_key,
        ":has_cover_art": cover,
        ":now": stamp,
    })?)
}
