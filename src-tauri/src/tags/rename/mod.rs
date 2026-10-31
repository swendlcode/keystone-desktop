//! Journal-first renames.
//!
//! **Renaming is the only irreversible operation in the app**, and one setting
//! runs it automatically after processing. So: nothing moves without a plan the
//! user can read first, nothing moves without a journal row, and every item
//! that will be skipped says why — the preview is the undo.
//!
//! Four hazards, each of which produces silent wrong behaviour rather than an
//! error, and each of which owes a test:
//!
//! - **Case-only renames on a case-insensitive filesystem** — [`paths`].
//! - **NFD vs NFC** — [`paths`].
//! - **Idempotence**: `Track - 8A - 128 - 8A - 128` must be impossible —
//!   [`compose`].
//! - **Windows** illegal characters, reserved device names, and the 255-byte
//!   component limit, which truncates the *Original* segment and never the key
//!   or tempo suffix — [`compose`].

pub mod compose;
pub mod journal;
pub mod paths;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use compose::{
    compose_stem, sanitize_component, strip_stem, truncate_component, MAX_COMPONENT_BYTES,
};
pub use paths::{is_case_only_change, is_same_file};

use crate::analysis::camelot::key_string;
use crate::db::TrackRepository;
use crate::error::Result;
use crate::models::{
    RenamePattern, RenamePlan, RenamePlanItem, RenameReport, Settings, TagWriteFailure, Track,
};
use crate::tags::MARKER_SEPARATOR;

/// Builds the plan. Nothing moves. Every item that will be skipped carries the
/// reason, because the preview is the undo.
///
/// `rename_mode` is deliberately **not** consulted: the Rename Files screen
/// shows this plan whether or not the automatic run is switched on, and the
/// caller decides whether to apply it.
pub fn plan(tracks: &[Track], settings: &Settings) -> Result<RenamePlan> {
    let mut claimed: HashSet<String> = HashSet::new();
    let items = tracks
        .iter()
        .map(|track| plan_item(track, settings, &mut claimed))
        .collect();
    Ok(RenamePlan { items })
}

fn plan_item(track: &Track, settings: &Settings, claimed: &mut HashSet<String>) -> RenamePlanItem {
    let from = PathBuf::from(&track.path);
    let skip = |reason: &str| RenamePlanItem {
        track_id: track.id.clone(),
        from_path: track.path.clone(),
        to_path: track.path.clone(),
        skipped: Some(reason.to_string()),
    };

    let Some(key) = track.key_root.zip(track.key_mode).map(|(root, mode)| {
        key_string(root, mode, settings.key_notation, settings.pad_camelot_numbers)
    }) else {
        return skip("No key detected yet.");
    };
    let tempo = integer_tempo(track);
    if tempo.is_none() && pattern_needs_tempo(settings.rename_pattern) {
        // Renaming twice is renaming twice, and renaming is irreversible. Wait
        // for the tempo rather than writing a name that has to be corrected.
        return skip("No tempo detected yet.");
    }

    let Some(to) = target(&from, track, settings, &key, tempo.as_deref().unwrap_or("")) else {
        return skip("This track has no parent directory.");
    };

    if paths::same_path_text(&from, &to) {
        return skip("Already named correctly.");
    }
    // The existence check has to come *after* the case-only test: on APFS the
    // target of a case-only rename always exists, because it is this file.
    if !is_case_only_change(&from, &to) && to.exists() {
        return skip("A different file already has that name.");
    }
    if !claimed.insert(paths::nfc(&to.to_string_lossy()).to_lowercase()) {
        return skip("Another track in this batch would take the same name.");
    }

    RenamePlanItem {
        track_id: track.id.clone(),
        from_path: track.path.clone(),
        to_path: to.to_string_lossy().into_owned(),
        skipped: None,
    }
}

/// Composes the target path: strip back to the original stem, apply the
/// pattern, sanitise, then fit inside the component limit without touching the
/// key, the tempo or the extension.
fn target(
    from: &Path,
    track: &Track,
    settings: &Settings,
    key: &str,
    tempo: &str,
) -> Option<PathBuf> {
    let name = from.file_name()?.to_string_lossy().into_owned();
    let ext = compose::ext_of(&name).to_string();
    let original = strip_stem(compose::stem_of(&name), track.original_filename.as_deref());

    let pattern = settings.rename_pattern;
    let block = marker_order(pattern, key, tempo)
        .into_iter()
        .filter(|m| !m.is_empty())
        .collect::<Vec<_>>()
        .join(MARKER_SEPARATOR);

    // `compose_stem` owns the arrangement and the idempotence guard; this only
    // decides which end of the name the length limit is allowed to eat.
    let stem = compose_stem(&original, pattern, key, tempo);
    let component = if block.is_empty() || stem == original {
        fit(&original, "", &ext)
    } else if pattern_puts_markers_last(pattern) {
        fit(&original, "", &format!("{MARKER_SEPARATOR}{block}{ext}"))
    } else {
        fit(&original, &format!("{block}{MARKER_SEPARATOR}"), &ext)
    };

    Some(from.with_file_name(component))
}

/// `prefix` and `suffix` are what must survive; only `original` is cut.
fn fit(original: &str, prefix: &str, suffix: &str) -> String {
    let budget = MAX_COMPONENT_BYTES.saturating_sub(prefix.len());
    let tail = truncate_component(original, suffix, budget);
    sanitize_component(&format!("{prefix}{tail}"))
}

fn marker_order<'a>(pattern: RenamePattern, key: &'a str, tempo: &'a str) -> Vec<&'a str> {
    match pattern {
        RenamePattern::OriginalKey | RenamePattern::KeyOriginal => vec![key],
        RenamePattern::OriginalKeyTempo | RenamePattern::KeyTempoOriginal => vec![key, tempo],
        RenamePattern::TempoKeyOriginal => vec![tempo, key],
    }
}

fn pattern_puts_markers_last(pattern: RenamePattern) -> bool {
    matches!(
        pattern,
        RenamePattern::OriginalKey | RenamePattern::OriginalKeyTempo
    )
}

fn pattern_needs_tempo(pattern: RenamePattern) -> bool {
    matches!(
        pattern,
        RenamePattern::OriginalKeyTempo
            | RenamePattern::KeyTempoOriginal
            | RenamePattern::TempoKeyOriginal
    )
}

fn integer_tempo(track: &Track) -> Option<String> {
    track
        .tempo_display
        .or(track.tempo_raw)
        .filter(|b| b.is_finite() && *b > 0.0)
        .map(|b| format!("{}", b.round() as i64))
}

/// Applies a plan, journal-first, one file at a time.
///
/// Records no `rename_pattern`, because a [`RenamePlan`] does not carry one.
/// Prefer [`apply_with_pattern`]: that column is what a later Remove Key from
/// File Name reads to know what was done.
pub fn apply(plan: &RenamePlan, repo: &Arc<TrackRepository>) -> Result<RenameReport> {
    run(plan, repo, None)
}

pub fn apply_with_pattern(
    plan: &RenamePlan,
    repo: &Arc<TrackRepository>,
    pattern: RenamePattern,
) -> Result<RenameReport> {
    run(plan, repo, Some(pattern_key(pattern)))
}

fn run(
    plan: &RenamePlan,
    repo: &Arc<TrackRepository>,
    pattern: Option<&'static str>,
) -> Result<RenameReport> {
    let mut report = RenameReport::default();
    for item in &plan.items {
        if item.skipped.is_some() {
            report.skipped += 1;
            continue;
        }
        match rename_one(repo, item, pattern) {
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

fn rename_one(
    repo: &Arc<TrackRepository>,
    item: &RenamePlanItem,
    pattern: Option<&'static str>,
) -> Result<()> {
    // The row goes in first. Everything after this point is recoverable; a
    // rename without it is not.
    let op = journal::open(repo, &item.track_id, &item.from_path, &item.to_path)?;
    let result = move_and_record(repo, item, pattern, &op);
    // The row is cleared whatever happened: on failure nothing moved, and on
    // success the database is already in step. A row left behind would make the
    // next launch try to repair a rename that is finished.
    let _ = journal::close(repo, &op);
    result
}

fn move_and_record(
    repo: &Arc<TrackRepository>,
    item: &RenamePlanItem,
    pattern: Option<&'static str>,
    op: &str,
) -> Result<()> {
    paths::move_file(Path::new(&item.from_path), Path::new(&item.to_path))?;
    journal::mark_renamed(repo, op)?;
    let original = Path::new(&item.from_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    // `update_path` COALESCEs `original_filename`, so a second rename cannot
    // overwrite the name the first one started from — which is what keeps the
    // inverse strip exact rather than a regex guess.
    repo.update_path(&item.track_id, &item.to_path, original.as_deref(), pattern)
}

/// Replays `file_ops` rows left behind by a crash. Called on startup.
pub fn recover_journal(repo: &Arc<TrackRepository>) -> Result<usize> {
    journal::recover(repo)
}

/// The stored spelling of the pattern, matching the camelCase serde
/// representation the frontend already uses for `RenamePattern`.
fn pattern_key(pattern: RenamePattern) -> &'static str {
    match pattern {
        RenamePattern::OriginalKey => "originalKey",
        RenamePattern::OriginalKeyTempo => "originalKeyTempo",
        RenamePattern::KeyOriginal => "keyOriginal",
        RenamePattern::KeyTempoOriginal => "keyTempoOriginal",
        RenamePattern::TempoKeyOriginal => "tempoKeyOriginal",
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
