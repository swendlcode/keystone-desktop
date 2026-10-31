use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{Emitter, State};

use crate::analysis::camelot;
use crate::commands::blocking;
use crate::db::{CueRepository, TrackRepository};
use crate::error::{KeystoneError, Result};
use crate::events::{self, FileProgressEvent};
use crate::export::{
    self, detect, safety, ExportContext, ExportReport, ExportRequest, ExportTarget, ExportTrack,
    TargetStatus,
};
use crate::models::Settings;
use crate::state::AppState;

/// Detection is best-effort and each status carries the sentence its disabled
/// checkbox shows, plus the manual steps the user must take afterwards.
/// rekordbox cannot be pushed into and Serato needs Rescan ID3 Tags; leaving
/// those unsaid is how the feature gets reported as broken by people for whom
/// it worked perfectly.
#[tauri::command]
pub async fn detect_export_targets(state: State<'_, AppState>) -> Result<Vec<TargetStatus>> {
    let settings = state.settings.read().clone();
    // Detection stats paths and probes for running processes, so it does not
    // belong on the async runtime's thread.
    blocking(move || Ok(export::detect_targets(&settings))).await
}

/// Backs the spec's `CHANGE` button, which is mandatory rather than a nicety:
/// Traktor's root moves on every version update and is user-overridable.
#[tauri::command]
pub async fn set_export_path(
    target: ExportTarget,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    if let Some(p) = path.as_deref().filter(|p| !Path::new(p).exists()) {
        return Err(KeystoneError::NotFound(format!("{p} is not there")));
    }
    {
        let mut settings = state.settings.write();
        match target {
            ExportTarget::Serato => settings.serato_path = path,
            ExportTarget::Traktor => settings.traktor_path = path,
            ExportTarget::Rekordbox => settings.rekordbox_path = path,
        }
    }
    state.save_settings()
}

/// Dry run by default. Shows what would change before anything is written.
#[tauri::command]
pub async fn dry_run_export(
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<ExportReport> {
    let tracks = state.tracks.clone();
    let cues = state.cues.clone();
    let settings = state.settings.read().clone();
    let backups_dir = state.backups_dir.clone();
    blocking(move || {
        let assembled = assemble(&tracks, &cues, &settings, &request.track_ids)?;
        let ctx = ExportContext {
            backups_dir: &backups_dir,
            settings: &settings,
        };
        export::dry_run(&request, &assembled, &ctx)
    })
    .await
}

/// Refuses while the target app is running — a blocking modal, not a
/// dismissible warning, because Traktor holds its collection in memory and
/// rewrites `collection.nml` on exit, which silently discards the edit and can
/// launder a half-written file into the backup rotation.
///
/// The refusal, the dated backup and the atomic write all live in
/// [`crate::export`], which is the only place a target writer is reached from.
/// This handler assembles the projection and reports.
#[tauri::command]
pub async fn export_cue_points(
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<ExportReport> {
    let tracks = state.tracks.clone();
    let cues = state.cues.clone();
    let settings = state.settings.read().clone();
    let backups_dir = state.backups_dir.clone();
    let app = state.app.clone();
    blocking(move || {
        let assembled = assemble(&tracks, &cues, &settings, &request.track_ids)?;
        let ctx = ExportContext {
            backups_dir: &backups_dir,
            settings: &settings,
        };
        let report = export::export(&request, &assembled, &ctx)?;
        let _ = app.emit(events::EXPORT_PROGRESS, finished(&report));
        Ok(report)
    })
    .await
}

/// The only thing that matters when it goes wrong, which is why it is in
/// Settings rather than hidden.
#[tauri::command]
pub async fn list_export_backups(
    target: ExportTarget,
    state: State<'_, AppState>,
) -> Result<Vec<safety::BackupEntry>> {
    let backups_dir = state.backups_dir.clone();
    blocking(move || safety::list_backups(&backups_dir, target)).await
}

#[tauri::command]
pub async fn restore_export_backup(
    target: ExportTarget,
    backup_path: String,
    state: State<'_, AppState>,
) -> Result<()> {
    let settings = state.settings.read().clone();
    blocking(move || {
        let dest = destination(target, &settings)?;
        // The same refusal as a write. Restoring under a running Traktor is
        // overwritten again the moment it exits, which looks like the restore
        // silently failed.
        safety::assert_not_running(target, Some(&dest))?;
        safety::restore_backup(Path::new(&backup_path), &dest)
    })
    .await
}

/// The projection the exporters read, assembled once so no exporter reaches for
/// a repository.
fn assemble(
    tracks: &Arc<TrackRepository>,
    cues: &Arc<CueRepository>,
    settings: &Settings,
    ids: &[String],
) -> Result<Vec<ExportTrack>> {
    let mut out = Vec::with_capacity(ids.len());
    for track in tracks.get_many(ids)? {
        // Formatted once, here. Each target wants a different field and none of
        // them should re-decide notation.
        let key_string = match (track.key_root, track.key_mode) {
            (Some(root), Some(mode)) => camelot::key_string(
                root,
                mode,
                settings.key_notation,
                settings.pad_camelot_numbers,
            ),
            _ => String::new(),
        };
        out.push(ExportTrack {
            cues: cues.list_for_track(&track.id)?,
            beatgrid: cues.beatgrid(&track.id)?,
            key_string,
            track,
        });
    }
    Ok(out)
}

/// One closing progress event. Per-file progress would need a callback on
/// `export::export`, and a streamed rewrite does not have per-file moments to
/// report anyway: the whole collection is one atomic write.
fn finished(report: &ExportReport) -> FileProgressEvent {
    FileProgressEvent {
        run_id: report.run_id.clone(),
        done: report.written + report.skipped + report.failed,
        total: report.planned,
        failed: report.failed,
        current_path: report.destination.clone(),
    }
}

/// Where a restore writes back to: the user's override, else detection.
fn destination(target: ExportTarget, settings: &Settings) -> Result<PathBuf> {
    let configured = match target {
        ExportTarget::Serato => settings.serato_path.clone(),
        ExportTarget::Traktor => settings.traktor_path.clone(),
        ExportTarget::Rekordbox => settings.rekordbox_path.clone(),
    };
    configured
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| detect::default_destination(target))
        .ok_or_else(|| {
            KeystoneError::ExportRefused(format!(
                "there is no {} library to restore over; choose it with CHANGE first",
                target.as_str()
            ))
        })
}
