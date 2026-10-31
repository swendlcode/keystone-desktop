//! Journal-first renames.
//!
//! **Renaming is the only irreversible operation in the app**, and one setting
//! runs it automatically after processing. The journal row exists before
//! `fs::rename` and is cleared after the database UPDATE, so a crash in the
//! middle is recoverable rather than a row pointing at a path that no longer
//! exists.
//!
//! Four hazards, each of which produces silent wrong behaviour rather than an
//! error, and each of which owes a unit test:
//!
//! - **Case-only renames on a case-insensitive filesystem.** APFS is
//!   case-insensitive by default, so `track.mp3` -> `Track.mp3` is a no-op or an
//!   error depending on the call. Go via a temporary name.
//! - **NFD vs NFC.** macOS stores decomposed filenames, so a string compare
//!   against a composed name says "different file" when it is the same file.
//!   Compare normalised, and use `same-file` for identity rather than comparing
//!   path strings at all.
//! - **Idempotence.** `Track - 8A - 128 - 8A - 128.mp3` must be impossible.
//!   `original_filename` is what makes the strip exact instead of a regex guess.
//! - **Windows.** Illegal characters, reserved device names (`CON`, `NUL`,
//!   `COM1`..), and the 255-byte component limit — which must truncate the
//!   *Original* segment, never the key/tempo suffix, or the rename defeats its
//!   own purpose.

use std::path::Path;
use std::sync::Arc;

use unicode_normalization::UnicodeNormalization;

use crate::db::{FileOp, TrackRepository};
use crate::error::Result;
use crate::models::{RenamePattern, RenamePlan, RenameReport, Settings, Track};

/// Maximum bytes in one path component on the filesystems we target.
pub const MAX_COMPONENT_BYTES: usize = 255;

/// Builds the plan. Nothing moves. Every item that will be skipped carries the
/// reason, because the preview is the undo.
pub fn plan(_tracks: &[Track], _settings: &Settings) -> Result<RenamePlan> {
    todo!("owes: compose each target name, detect collisions, mark skips with reasons")
}

/// Applies a plan, journal-first, one file at a time.
pub fn apply(_plan: &RenamePlan, _repo: &Arc<TrackRepository>) -> Result<RenameReport> {
    todo!("owes: write the file_ops row, rename, UPDATE the track, clear the row")
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

/// Settles one journal row. `true` when the database had to be corrected.
fn replay(repo: &Arc<TrackRepository>, op: &FileOp) -> Result<bool> {
    let from = Path::new(&op.from_path);
    let to = Path::new(&op.to_path);
    let landed = match outcome(from, to) {
        Some(Outcome::Renamed) => &op.to_path,
        Some(Outcome::NotRenamed) => &op.from_path,
        None => {
            // Neither path is on disk: the file was moved or deleted outside the
            // app while the journal row was open. Nothing here can tell which
            // side won, and guessing would point the row at a path that does not
            // exist. Reconciliation marks it missing instead.
            tracing::warn!(
                "rename recovery: neither {} nor {} exists; left for reconciliation",
                op.from_path,
                op.to_path
            );
            return Ok(false);
        }
    };
    let Some(track) = repo.get_by_id(&op.track_id)? else {
        // The track was removed from the library after the journal row was
        // written. The row is stale, not a disagreement.
        return Ok(false);
    };
    if same_path(&track.path, landed) {
        return Ok(false);
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
    let want = nfc(&name.to_string_lossy());
    match std::fs::read_dir(parent) {
        Ok(entries) => entries
            .flatten()
            .any(|e| nfc(&e.file_name().to_string_lossy()) == want),
        Err(_) => false,
    }
}

/// Path equality for comparing a stored path against one from the journal:
/// NFC-normalised, so an NFD spelling of the same name is not a difference.
fn same_path(a: &str, b: &str) -> bool {
    a == b || nfc(a) == nfc(b)
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Composes the new stem from the pattern. `key` and `tempo` are already
/// formatted by the Key Notation and Tempo settings; this function does not
/// re-decide notation.
pub fn compose_stem(
    _original: &str,
    _pattern: RenamePattern,
    _key: &str,
    _tempo: &str,
) -> String {
    todo!("owes: the five patterns from SPEC.md §3")
}

/// The exact inverse, using `original_filename` rather than a pattern match.
/// Backs the context menu's Remove Key from File Name.
pub fn strip_stem(_current: &str, _original_filename: Option<&str>) -> String {
    todo!("owes: restore the original stem exactly, or strip a trailing key/tempo suffix")
}

/// Replaces characters the target filesystem rejects and escapes reserved
/// device names.
pub fn sanitize_component(_name: &str) -> String {
    todo!("owes: strip <>:\"/\\|?* and control chars; escape CON, NUL, COM1.., LPT1..")
}

/// Truncates to fit `MAX_COMPONENT_BYTES`, taking the bytes off the `original`
/// segment and never off `suffix`.
pub fn truncate_component(_original: &str, _suffix: &str, _max_bytes: usize) -> String {
    todo!("owes: UTF-8-safe truncation of the original segment only")
}

/// Identity, not string equality. Uses `same-file` so a hard link, a symlink and
/// an NFD/NFC spelling of the same file all compare equal.
pub fn is_same_file(a: &Path, b: &Path) -> bool {
    match same_file::is_same_file(a, b) {
        Ok(same) => same,
        // One of them could not be opened — most often because it does not
        // exist. Fall back to the normalised string compare, which is the only
        // thing left to go on and is still right for an NFD/NFC pair.
        Err(_) => same_path(&a.to_string_lossy(), &b.to_string_lossy()),
    }
}

/// True when the two names differ only by case, which needs the two-step
/// rename on a case-insensitive filesystem.
pub fn is_case_only_change(_from: &Path, _to: &Path) -> bool {
    todo!("owes: case-insensitive equality but case-sensitive inequality")
}

#[cfg(test)]
#[path = "renamer_tests.rs"]
mod tests;
