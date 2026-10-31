//! Running one edit over many files, and reporting it honestly.
//!
//! Every bulk tag operation in SPEC.md — RE-TAG EXISTING FILES, Global Replace,
//! Remove Extra Spaces, Remove Key from Common Tags — is the same loop with a
//! different edit, so the loop lives here and the commands pass a closure. That
//! is also what keeps the commands thin enough to stay thin.
//!
//! Two things it does that a naive loop would not:
//!
//! - **An empty edit is a skip, not a write.** Rewriting a file that needs no
//!   change costs the user an mtime, and the DJ apps re-analyse on mtime. The
//!   count the confirm step shows has to mean something too.
//! - **The row is brought into step with the file afterwards.** The file owns
//!   its tags; the row is a cache of them for the table. Leaving the row stale
//!   makes the library view disagree with the file the user just edited.

use std::path::Path;
use std::sync::Arc;

use super::write::{self, WriteOptions};
use crate::db::{DatabasePool, TrackRepository};
use crate::error::Result;
use crate::models::{TagEdit, TagSnapshot, TagWriteFailure, TagWriteReport, Track};

/// Called after each file with (done, total) so the command can emit
/// `keystone://tag-progress`. Kept as a callback because the core has no Tauri
/// dependency and must not grow one.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize);

pub struct Batch<'a> {
    pub db: &'a Arc<DatabasePool>,
    pub tracks: &'a Arc<TrackRepository>,
    /// The confirm step: count what would change and touch nothing.
    pub dry_run: bool,
}

impl Batch<'_> {
    /// Builds and applies one edit per track.
    ///
    /// `edit_for` sees the snapshot read from the file, not the database row,
    /// because the file is the source of truth for tags.
    pub fn run<F>(
        &self,
        tracks: &[Track],
        progress: Progress<'_>,
        mut edit_for: F,
    ) -> Result<TagWriteReport>
    where
        F: FnMut(&Track, &TagSnapshot) -> Result<TagEdit>,
    {
        let mut report = TagWriteReport {
            run_id: uuid::Uuid::new_v4().to_string(),
            written: 0,
            verified: 0,
            skipped: 0,
            failed: 0,
            failures: Vec::new(),
        };

        for (done, track) in tracks.iter().enumerate() {
            match self.one(track, &mut edit_for) {
                Ok(Outcome::Skipped) => report.skipped += 1,
                Ok(Outcome::Written { verified }) => {
                    report.written += 1;
                    report.verified += usize::from(verified);
                }
                Err(e) => {
                    report.failed += 1;
                    report.failures.push(TagWriteFailure {
                        track_id: track.id.clone(),
                        path: track.path.clone(),
                        reason: e.to_string(),
                    });
                }
            }
            progress(done + 1, tracks.len());
        }
        Ok(report)
    }

    fn one<F>(&self, track: &Track, edit_for: &mut F) -> Result<Outcome>
    where
        F: FnMut(&Track, &TagSnapshot) -> Result<TagEdit>,
    {
        let path = Path::new(&track.path);
        let snapshot = super::read::read(path)?;
        let edit = edit_for(track, &snapshot)?;
        if edit.fields.is_empty() {
            return Ok(Outcome::Skipped);
        }
        if self.dry_run {
            // Counted as written, because the count answers "how many files
            // would change" — which is the question the confirm step asks.
            return Ok(Outcome::Written { verified: false });
        }

        let report = write::write_with_backup(
            self.db,
            &track.id,
            path,
            &edit,
            &WriteOptions::default(),
        )?;
        // Re-read rather than guessing from the edit: what the container
        // actually accepted is what the table should show.
        self.tracks.update_tags(&track.id, &super::read::read(path)?)?;
        Ok(Outcome::Written { verified: report.verified })
    }
}

enum Outcome {
    Skipped,
    Written { verified: bool },
}
