use tauri::State;

use crate::commands::blocking;
use crate::error::{KeystoneError, Result};
use crate::state::{jobs::player, AppState};

/// Returns a path. The frontend plays it through `convertFileSrc` and the asset
/// protocol — no decoding happens on this path, because the player must not
/// compete with the analysis queue for CPU.
#[tauri::command]
pub async fn get_playback_path(track_id: String, state: State<'_, AppState>) -> Result<String> {
    let tracks = state.tracks.clone();
    blocking(move || player::playback_path(&tracks, &track_id)).await
}

/// The spec's "Start playing the track 25% of the way in", resolved against the
/// track's own duration so the frontend does not have to.
///
/// Whether playback starts at all is the separate `playAutomatically` setting,
/// which the frontend already has from `get_settings`.
#[tauri::command]
pub async fn get_start_offset_ms(track_id: String, state: State<'_, AppState>) -> Result<f64> {
    let tracks = state.tracks.clone();
    let settings = state.settings.read().clone();
    blocking(move || {
        let track = tracks
            .get_by_id(&track_id)?
            .ok_or_else(|| KeystoneError::NotFound(format!("track {track_id}")))?;
        Ok(player::start_offset_ms(&track, &settings))
    })
    .await
}
