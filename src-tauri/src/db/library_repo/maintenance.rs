//! The two maintenance passes, and the difference between them is the whole
//! reason they are separate.
//!
//! Marking missing is soft and automatic. Cleaning is hard and manual, because
//! an unmounted external drive is the ordinary cause of a missing row and a DJ
//! library living on one is normal rather than exceptional.

use super::LibraryRepository;
use crate::core::peaks;
use crate::db::query_builder::path_upper_bound;
use crate::db::mapping::now;
use crate::error::Result;
use crate::models::CleanCacheReport;

impl LibraryRepository {
    /// Soft pass: marks rows missing whose files are gone. Does not delete.
    pub fn mark_missing_under(&self, path_prefix: &str) -> Result<usize> {
        let conn = self.db.get()?;
        let n = conn.execute(
            "UPDATE tracks SET index_status = 'missing', updated_at = ?3 \
             WHERE path >= ?1 AND path < ?2 AND index_status != 'missing'",
            rusqlite::params![path_prefix, path_upper_bound(path_prefix), now()],
        )?;
        Ok(n)
    }

    /// Hard pass, behind the Clean cache action: drops missing rows, orphaned
    /// analysis-cache entries and the peak files nothing references any more.
    pub fn clean_cache(&self, peaks_dir: &std::path::Path) -> Result<CleanCacheReport> {
        let conn = self.db.get()?;
        let tracks_removed = conn.execute("DELETE FROM tracks WHERE index_status = 'missing'", [])?;
        let analysis_rows_removed = conn.execute(
            "DELETE FROM analysis_cache WHERE content_hash NOT IN \
             (SELECT content_hash FROM tracks WHERE content_hash IS NOT NULL)",
            [],
        )?;

        // Read the surviving hashes *after* the deletes, or the prune keeps
        // files for rows that no longer exist.
        let live_hashes: Vec<String> = {
            let mut stmt =
                conn.prepare("SELECT DISTINCT content_hash FROM tracks WHERE content_hash IS NOT NULL")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        drop(conn);

        let (peak_files_removed, bytes_freed) = peaks::prune(peaks_dir, &live_hashes)?;

        Ok(CleanCacheReport {
            tracks_removed,
            analysis_rows_removed,
            peak_files_removed,
            bytes_freed,
        })
    }
}
