//! Journal-first renames.
//!
//! **Renaming is the only irreversible operation in the app**, and one setting
//! runs it automatically after processing. The journal row exists before
//! `fs::rename` and is cleared after the database UPDATE, so a crash in between
//! is recoverable rather than a row pointing at a path that is gone.
//!
//! Four hazards, each of which produces silent wrong behaviour rather than an
//! error, and each of which owes a test in `renamer_apply_tests.rs`:
//! case-only renames on a case-insensitive filesystem, which go via a staging
//! name ([`paths`], plus [`find_staged`] for the crash in between); NFD vs NFC,
//! where macOS hands back decomposed names and only a normalised compare says
//! they are one file ([`same_path`]); idempotence, since
//! `Track - 8A - 128 - 8A - 128.mp3` must be impossible and `original_filename`
//! is what makes the strip exact ([`compose_stem`]); and Windows' illegal
//! characters, reserved device names and 255-byte component limit, which
//! truncates the *Original* segment and never the key/tempo suffix
//! ([`sanitize_component`], [`truncate_component`]).
//!
//! The naming rules and the path hazards are implemented and tested once, in
//! [`crate::tags::rename::compose`] and [`crate::tags::rename::paths`], and
//! re-exported from here rather than written a second time: a second copy that
//! compiles is the hazard the project notes record against `analysis/chroma.rs`.
//!
//! What this module owns is the half that talks to the database: [`plan`] is the
//! wired entry point, [`apply`] is journal-first through the repository, and
//! [`recover_journal`] is what `lib.rs` replays at launch. The journal goes
//! through `db/track_repo/journal.rs` because all SQL belongs in `db/`, so
//! `tags/rename/journal.rs` — which predates it and writes `file_ops` with its
//! own inline SQL — is not called from anywhere here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::{FileOp, TrackRepository};
use crate::error::{KeystoneError, Result};
use crate::models::{RenamePlan, RenamePlanItem, RenameReport, Settings, TagWriteFailure, Track};
use crate::tags::rename::paths;

pub use crate::tags::rename::compose::{
    compose_stem, sanitize_component, strip_stem, truncate_component, MAX_COMPONENT_BYTES,
};
pub use crate::tags::rename::paths::{is_case_only_change, is_same_file};

/// The prefix [`paths::move_file`] parks a file under between the two steps of a
/// case-only rename. Spelled again because `paths` keeps its staging name
/// private; it wants exposing, since a drift makes an interrupted rename
/// unrecoverable in silence.
const STAGING_PREFIX: &str = ".keystone-";

/// Builds the plan. Nothing moves. Every item that will be skipped carries the
/// reason, because the preview is the undo.
///
/// The composition is [`crate::tags::rename::plan`]: the five patterns, the
/// idempotence guard, the collision rules and the Windows limits have one
/// implementation and one set of tests. This is the name the commands call.
pub fn plan(tracks: &[Track], settings: &Settings) -> Result<RenamePlan> {
    crate::tags::rename::plan(tracks, settings)
}

/// Applies a plan, journal-first, one file at a time and never in a
/// transaction: `fs::rename` is not transactional, so a batch that failed half
/// way through would be unwound by hand. Each file settles completely — row,
/// move, UPDATE, row cleared — before the next one starts.
pub fn apply(plan: &RenamePlan, repo: &Arc<TrackRepository>) -> Result<RenameReport> {
    let mut report = RenameReport::default();
    for item in &plan.items {
        if item.skipped.is_some() {
            report.skipped += 1;
            continue;
        }
        match rename_one(repo, item) {
            Ok(()) => report.renamed += 1,
            Err(e) => {
                report.failed += 1;
                report.failures.push(TagWriteFailure {
                    track_id: item.track_id.clone(),
                    path: item.from_path.clone(),
                    reason: e.to_string(),
                });
            }
        }
    }
    Ok(report)
}

fn rename_one(repo: &Arc<TrackRepository>, item: &RenamePlanItem) -> Result<()> {
    let Some(track) = repo.get_by_id(&item.track_id)? else {
        return Err(KeystoneError::NotFound(format!("track {}", item.track_id)));
    };
    // The preview is the undo, so a plan can be minutes old when it is
    // confirmed. If the row has moved on since, moving the file is a guess.
    if !same_path(&track.path, &item.from_path) {
        return Err(KeystoneError::Other(format!(
            "{} has moved since the preview was built",
            item.from_path
        )));
    }

    // The row goes in first. Everything after this point is recoverable; a
    // rename without it is not.
    let op = repo.record_file_op(&item.track_id, &item.from_path, &item.to_path)?;
    let result = move_and_record(repo, item, &track, &op);
    // Cleared whatever happened: on failure nothing moved, on success the
    // database is already in step. A failed DELETE is not a failed rename —
    // recovery finds the row, sees both sides agree and clears it.
    if let Err(e) = repo.clear_file_op(&op) {
        tracing::warn!("rename: journal row {} could not be cleared: {}", op, e);
    }
    result
}

fn move_and_record(
    repo: &Arc<TrackRepository>,
    item: &RenamePlanItem,
    track: &Track,
    op: &str,
) -> Result<()> {
    paths::move_file(Path::new(&item.from_path), Path::new(&item.to_path))?;
    repo.mark_file_op_renamed(op)?;
    let original = Path::new(&item.from_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    // `update_path` COALESCEs `original_filename`, so a second rename cannot
    // overwrite the name the first one started from — which is what keeps the
    // inverse strip exact rather than a regex guess.
    //
    // `rename_pattern` is passed back unchanged rather than recorded: a
    // `RenamePlan` carries no pattern, and a guess in the column that says which
    // one was applied is worse than the value already in it.
    repo.update_path(
        &item.track_id,
        &item.to_path,
        original.as_deref(),
        track.rename_pattern.as_deref(),
    )
}

/// Replays `file_ops` rows left behind by a crash. Called on startup, before
/// anything reads a path.
///
/// Returns the number of rows whose two sides actually disagreed, which is what
/// the launch log reports; rows that were already consistent are cleared
/// silently. The filesystem is the authority here, never the journal: the row
/// says what was *intended*, and only the disk says whether it happened.
pub fn recover_journal(repo: &Arc<TrackRepository>) -> Result<usize> {
    repo.purge_finished_file_ops()?;
    let mut repaired = 0usize;
    for op in repo.open_file_ops()? {
        if op.op != "rename" {
            // A row from a newer build. Replaying it would mean guessing what it
            // meant, so it is left alone and reported.
            tracing::warn!("rename recovery: unknown journal op {:?}, skipped", op.op);
            continue;
        }
        if replay(repo, &op)? {
            repaired += 1;
        }
        repo.clear_file_op(&op.id)?;
    }
    Ok(repaired)
}

/// Settles one journal row. `true` when something was corrected.
fn replay(repo: &Arc<TrackRepository>, op: &FileOp) -> Result<bool> {
    let Some((landed, moved)) = settle(op)? else {
        // Neither path is on disk and nothing is parked: the file was moved or
        // deleted outside the app while the journal row was open. Nothing here
        // can tell which side won, and guessing would point the row at a path
        // that does not exist. Reconciliation marks it missing instead.
        tracing::warn!(
            "rename recovery: neither {} nor {} exists; left for reconciliation",
            op.from_path,
            op.to_path
        );
        return Ok(false);
    };
    let Some(track) = repo.get_by_id(&op.track_id)? else {
        // The track was removed from the library after the journal row was
        // written. The row is stale, not a disagreement.
        return Ok(moved);
    };
    if same_path(&track.path, landed) {
        return Ok(moved);
    }
    let original = Path::new(&op.from_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    // `update_path` COALESCEs `original_filename`, so a re-run cannot overwrite
    // the real original with an already-renamed name, and the pattern is passed
    // back unchanged because the journal does not record which one was applied.
    repo.update_path(
        &track.id,
        landed,
        original.as_deref(),
        track.rename_pattern.as_deref(),
    )?;
    Ok(true)
}

/// Where the file is, and whether getting it there needed a move. `None` when
/// the filesystem cannot say.
fn settle<'a>(op: &'a FileOp) -> Result<Option<(&'a str, bool)>> {
    let from = Path::new(&op.from_path);
    let to = Path::new(&op.to_path);
    Ok(match outcome(from, to) {
        Some(Outcome::Renamed) => Some((op.to_path.as_str(), false)),
        Some(Outcome::NotRenamed) => Some((op.from_path.as_str(), false)),
        // Neither side exists: a case-only rename parks the file under a
        // staging name, and a crash in that window leaves it under a name
        // neither side of the row mentions.
        None => match find_staged(to) {
            Some(staged) => {
                std::fs::rename(&staged, to)?;
                Some((op.to_path.as_str(), true))
            }
            None => None,
        },
    })
}

/// Which side of the rename the filesystem is on. `None` means it cannot say.
enum Outcome {
    Renamed,
    NotRenamed,
}

fn outcome(from: &Path, to: &Path) -> Option<Outcome> {
    match (from.exists(), to.exists()) {
        (false, true) => Some(Outcome::Renamed),
        (true, false) => Some(Outcome::NotRenamed),
        (true, true) if is_same_file(from, to) => {
            // One file answering to both paths is a case-only or NFD/NFC
            // difference, where `exists()` cannot tell the two apart on APFS or
            // NTFS. The directory entry carries the spelling that actually won.
            if spelled_on_disk(to) {
                Some(Outcome::Renamed)
            } else {
                Some(Outcome::NotRenamed)
            }
        }
        (true, true) => {
            // Two different files. The source is still where it was, so the
            // rename did not happen; something else owns the target name.
            tracing::warn!(
                "rename recovery: {} already exists and is a different file; \
                 the rename to it did not happen",
                to.display()
            );
            Some(Outcome::NotRenamed)
        }
        (false, false) => None,
    }
}

/// Whether the parent directory lists `path` with exactly this spelling.
///
/// Compared NFC-normalised, because macOS stores decomposed names and a
/// composed comparison would report every accented filename as missing.
fn spelled_on_disk(path: &Path) -> bool {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let want = paths::nfc(&name.to_string_lossy());
    match std::fs::read_dir(parent) {
        Ok(entries) => entries
            .flatten()
            .any(|e| paths::nfc(&e.file_name().to_string_lossy()) == want),
        Err(_) => false,
    }
}

/// Looks for the file parked by the first half of a case-only rename.
///
/// Matched on the NFC-normalised target name, so a decomposed directory entry
/// still answers to a composed path out of the database.
fn find_staged(to: &Path) -> Option<PathBuf> {
    let (dir, name) = (to.parent()?, to.file_name()?);
    let wanted = paths::nfc(&name.to_string_lossy());
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let spelled = paths::nfc(&entry.file_name().to_string_lossy());
        let staged = spelled.starts_with(STAGING_PREFIX) && spelled.ends_with(&wanted);
        staged.then(|| entry.path())
    })
}

/// Path equality for a stored path against one from the journal: NFC-normalised,
/// so an NFD spelling of the same name is not a difference.
fn same_path(a: &str, b: &str) -> bool {
    a == b || paths::nfc(a) == paths::nfc(b)
}

#[cfg(test)]
#[path = "renamer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "renamer_apply_tests.rs"]
mod apply_tests;
