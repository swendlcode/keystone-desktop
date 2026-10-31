use tauri::State;

use crate::commands::blocking;
use crate::error::{KeystoneError, Result};
use crate::models::{BeatgridMarker, CueEdit, CuePoint};
use crate::state::AppState;

/// The eight CUE markers, by slot. What the player bar draws on the waveform.
#[tauri::command]
pub async fn get_cue_points(track_id: String, state: State<'_, AppState>) -> Result<Vec<CuePoint>> {
    let cues = state.cues.clone();
    blocking(move || cues.list_for_track(&track_id)).await
}

/// One query for every visible row's CUE POINTS column, not one per row.
#[tauri::command]
pub async fn get_cue_counts(
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<(String, usize)>> {
    let cues = state.cues.clone();
    blocking(move || cues.counts_for_tracks(&track_ids)).await
}

/// Writes `source = 'user'`, which protects the marker from the next analysis.
#[tauri::command]
pub async fn add_cue_point(edit: CueEdit, state: State<'_, AppState>) -> Result<CuePoint> {
    validate(edit.slot, edit.position_ms)?;
    if let Some(end) = edit.end_ms.filter(|e| *e <= edit.position_ms) {
        return Err(KeystoneError::Other(format!(
            "a loop cannot end at {end} ms when it starts at {} ms",
            edit.position_ms
        )));
    }
    let cues = state.cues.clone();
    blocking(move || cues.upsert_user(&edit)).await
}

/// Dragging a marker. Also flips `source` to `user`: once the DJ has moved it,
/// it is theirs.
#[tauri::command]
pub async fn move_cue_point(
    cue_id: String,
    position_ms: f64,
    state: State<'_, AppState>,
) -> Result<CuePoint> {
    if !position_ms.is_finite() || position_ms < 0.0 {
        return Err(KeystoneError::Other(format!(
            "{position_ms} is not a position in the track"
        )));
    }
    let cues = state.cues.clone();
    blocking(move || cues.move_cue(&cue_id, position_ms)).await
}

#[tauri::command]
pub async fn delete_cue_point(cue_id: String, state: State<'_, AppState>) -> Result<()> {
    let cues = state.cues.clone();
    blocking(move || cues.delete(&cue_id)).await
}

/// The context menu's Remove Cue Points. `include_user` is what separates it
/// from a re-analysis clearing only its own work.
#[tauri::command]
pub async fn remove_cue_points(
    track_id: String,
    include_user: bool,
    state: State<'_, AppState>,
) -> Result<usize> {
    let cues = state.cues.clone();
    blocking(move || cues.delete_for_track(&track_id, include_user)).await
}

#[tauri::command]
pub async fn get_beatgrid(
    track_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<BeatgridMarker>> {
    let cues = state.cues.clone();
    blocking(move || cues.beatgrid(&track_id)).await
}

/// Slots are 0-7 because there are eight markers, and the slot is also the A-H
/// hot cue every export target maps onto. A ninth would be written and then
/// silently dropped by rekordbox.
fn validate(slot: u8, position_ms: f64) -> Result<()> {
    if slot > 7 {
        return Err(KeystoneError::Other(format!(
            "there are eight cue slots; {slot} is not one of them"
        )));
    }
    if !position_ms.is_finite() || position_ms < 0.0 {
        return Err(KeystoneError::Other(format!(
            "{position_ms} is not a position in the track"
        )));
    }
    Ok(())
}
