use tauri::State;

use crate::analysis::version::ANALYSIS_VERSION;
use crate::commands::blocking;
use crate::core::JobPriority;
use crate::error::{KeystoneError, Result};
use crate::models::{AnalysisProgress, AnalysisResult, TrackRegion, Waveform};
use crate::state::{jobs::player, AppState};

/// Checks each content hash against `analysis_cache` first: a moved file, a file
/// Keystone itself renamed, and a duplicate all cost no DSP.
///
/// Returns the queue's state rather than a count, because the sidebar is what
/// the user is looking at when they press this.
#[tauri::command]
pub async fn enqueue_analysis(
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<AnalysisProgress> {
    let queue = state.queue.clone();
    blocking(move || {
        queue.enqueue(track_ids, JobPriority::Normal)?;
        Ok(queue.progress())
    })
    .await
}

/// Everything the library still owes, from one indexed query. The same call the
/// queue makes at launch, which is what makes a run resumable.
#[tauri::command]
pub async fn enqueue_all_unanalyzed(state: State<'_, AppState>) -> Result<AnalysisProgress> {
    let queue = state.queue.clone();
    blocking(move || {
        queue.enqueue_pending()?;
        Ok(queue.progress())
    })
    .await
}

/// The context menu's Re-analyze Song. Bypasses the cache on purpose.
#[tauri::command]
pub async fn reanalyze_tracks(
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<AnalysisProgress> {
    let queue = state.queue.clone();
    blocking(move || {
        queue.force_reanalyze(track_ids)?;
        Ok(queue.progress())
    })
    .await
}

#[tauri::command]
pub async fn get_analysis_progress(state: State<'_, AppState>) -> Result<AnalysisProgress> {
    Ok(state.queue.progress())
}

#[tauri::command]
pub async fn pause_analysis(state: State<'_, AppState>) -> Result<AnalysisProgress> {
    state.queue.pause();
    Ok(state.queue.progress())
}

#[tauri::command]
pub async fn resume_analysis(state: State<'_, AppState>) -> Result<AnalysisProgress> {
    state.queue.resume();
    Ok(state.queue.progress())
}

/// Drains what is waiting. Work already committed stays committed, and the
/// cancelled tracks are still unanalysed, so the next launch offers them again.
#[tauri::command]
pub async fn cancel_analysis(state: State<'_, AppState>) -> Result<AnalysisProgress> {
    state.queue.cancel();
    Ok(state.queue.progress())
}

/// Served from the on-disk cache keyed by `content_hash`.
///
/// `None` is a normal answer: the decode is scheduled on the analysis pool and
/// the frontend redraws on `WAVEFORM_READY`. Decoding here instead would take
/// seconds of CPU from the queue the user is already watching, every time they
/// clicked a row.
#[tauri::command]
pub async fn get_waveform(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Option<Waveform>> {
    let tracks = state.tracks.clone();
    let queue = state.queue.clone();
    let peaks_dir = state.peaks_dir.clone();
    blocking(move || {
        let track = tracks
            .get_by_id(&track_id)?
            .ok_or_else(|| KeystoneError::NotFound(format!("track {track_id}")))?;
        if let Some(waveform) = player::cached_waveform(&peaks_dir, &track)? {
            return Ok(Some(waveform));
        }
        queue.request_peaks(&track);
        Ok(None)
    })
    .await
}

/// Per-section energy and key. Stored in v1 and read by the player's energy
/// overlay; the deferred deck view reads the same rows.
#[tauri::command]
pub async fn get_track_regions(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TrackRegion>> {
    let analysis = state.analysis.clone();
    blocking(move || analysis.regions(&track_id)).await
}

/// The whole cached payload for one track, for the Song Info panel.
///
/// Keyed by content hash, so a track whose file has moved still answers.
#[tauri::command]
pub async fn get_analysis(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Option<AnalysisResult>> {
    let tracks = state.tracks.clone();
    let analysis = state.analysis.clone();
    blocking(move || {
        let Some(track) = tracks.get_by_id(&track_id)? else {
            return Ok(None);
        };
        let Some(hash) = track.content_hash else {
            return Ok(None);
        };
        analysis.lookup(&hash, ANALYSIS_VERSION)
    })
    .await
}
