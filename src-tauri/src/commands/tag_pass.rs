//! One shape for every operation that writes to the user's files.
//!
//! Global Replace, Remove Extra Spaces, Re-tag and Remove Key all do the same
//! four things per track: read the tags, decide an edit, write it through the
//! safe path, and mirror the result into the row. Keeping that in one place is
//! what lets the handlers in [`super::tag_commands`] stay thin, and means the
//! dry run, the progress event and the per-file failure list cannot drift
//! between them.

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::db::TrackRepository;
use crate::error::Result;
use crate::events::{self, FileProgressEvent};
use crate::models::{TagEdit, TagSnapshot, TagWriteFailure, TagWriteReport, Track, TrackQuery};
use crate::state::AppState;
use crate::tags;

/// Files per progress event. A per-file emit over a 5,000-track library floods
/// the webview, and the progress bar only renders a fraction.
const EMIT_EVERY: usize = 25;

/// Rows per page when a pass covers the whole library.
const PAGE: usize = 5_000;

pub struct TagPass {
    tracks: Arc<TrackRepository>,
    app: AppHandle,
    run_id: String,
    /// Dry run by default at every call site: this writes to irreplaceable
    /// files, so the count is shown before anything is touched.
    dry_run: bool,
}

enum Outcome {
    /// Nothing to change on this file.
    Skipped,
    /// A dry run: what would have been written.
    Planned,
    Written { verified: bool },
}

impl TagPass {
    pub fn new(state: &AppState, dry_run: bool) -> Self {
        Self {
            tracks: state.tracks.clone(),
            app: state.app.clone(),
            run_id: uuid::Uuid::new_v4().to_string(),
            dry_run,
        }
    }

    /// Runs `edit_for` over each track. Returning `None` from it is a skip, not
    /// a failure: a file with nothing to change is the normal case on a second
    /// pass, which is what makes RE-TAG EXISTING FILES safe to re-run.
    pub fn run<F>(&self, ids: &[String], mut edit_for: F) -> Result<TagWriteReport>
    where
        F: FnMut(&Track, &TagSnapshot) -> Result<Option<TagEdit>>,
    {
        let rows = self.tracks.get_many(ids)?;
        let total = rows.len();
        let mut report = self.empty_report();
        for (done, track) in rows.iter().enumerate() {
            match self.one(track, &mut edit_for) {
                Ok(Outcome::Skipped) => report.skipped += 1,
                Ok(Outcome::Planned) => report.written += 1,
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
            self.emit(done + 1, total, report.failed, &track.path);
        }
        Ok(report)
    }

    pub fn empty_report(&self) -> TagWriteReport {
        TagWriteReport {
            run_id: self.run_id.clone(),
            written: 0,
            verified: 0,
            skipped: 0,
            failed: 0,
            failures: Vec::new(),
        }
    }

    fn one<F>(&self, track: &Track, edit_for: &mut F) -> Result<Outcome>
    where
        F: FnMut(&Track, &TagSnapshot) -> Result<Option<TagEdit>>,
    {
        let path = Path::new(&track.path);
        let snapshot = tags::read::read(path)?;
        let Some(edit) = edit_for(track, &snapshot)? else {
            return Ok(Outcome::Skipped);
        };
        if self.dry_run {
            return Ok(Outcome::Planned);
        }
        let written = tags::write::write(path, &edit, &tags::write::WriteOptions::default())?;
        // The write restores the original mtime, deliberately — otherwise every
        // DJ app re-analyses the library because Keystone re-tagged it. That
        // means the row is the only thing left that knows the tags moved, so
        // mirroring it is not an optimisation.
        self.tracks.update_tags(&track.id, &tags::read::read(path)?)?;
        Ok(Outcome::Written {
            verified: written.verified,
        })
    }

    fn emit(&self, done: usize, total: usize, failed: usize, path: &str) {
        if done % EMIT_EVERY != 0 && done != total {
            return;
        }
        let _ = self.app.emit(
            events::TAG_PROGRESS,
            FileProgressEvent {
                run_id: self.run_id.clone(),
                done,
                total,
                failed,
                current_path: Some(path.to_string()),
            },
        );
    }
}

/// Every live track's id, paged.
///
/// Used where the user asked for the whole library — RE-TAG EXISTING FILES, and
/// Global Replace with no selection. Paged because one `SELECT` of 160,000 rows
/// to collect their ids is a stall the user would attribute to the write.
pub fn all_track_ids(tracks: &Arc<TrackRepository>) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut offset = 0;
    loop {
        let page = tracks.search(&TrackQuery {
            filters: Default::default(),
            sort: crate::models::TrackSort::Filename,
            direction: crate::models::SortDirection::Asc,
            limit: PAGE,
            offset,
        })?;
        let got = page.tracks.len();
        ids.extend(page.tracks.into_iter().map(|t| t.id));
        if got < PAGE {
            return Ok(ids);
        }
        offset += got;
    }
}
