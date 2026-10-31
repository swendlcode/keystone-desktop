use tauri::State;

use crate::commands::blocking;
use crate::error::Result;
use crate::models::Settings;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(state.settings.read().clone())
}

/// Persists, then applies the consequences: a changed analysis concurrency
/// resizes the pool, and a changed tempo range re-folds the display column.
///
/// The file is written before the consequences are applied. A crash between the
/// two costs a pool width until the next launch; a crash the other way round
/// would leave the app running settings the user cannot see.
#[tauri::command]
pub async fn update_settings(settings: Settings, state: State<'_, AppState>) -> Result<Settings> {
    let previous = {
        let mut guard = state.settings.write();
        std::mem::replace(&mut *guard, settings)
    };
    state.save_settings()?;
    if let Some((lo, hi)) = state.apply_settings(&previous) {
        let tracks = state.tracks.clone();
        blocking(move || tracks.refold_tempo_display(lo, hi)).await?;
    }
    Ok(state.settings.read().clone())
}

/// Re-folds `tempo_display` from `tempo_raw` across the library.
///
/// No file is read and no DSP runs: that is the entire reason the three tempo
/// columns are stored separately.
#[tauri::command]
pub async fn refold_tempo_display(state: State<'_, AppState>) -> Result<usize> {
    let (lo, hi) = state.settings.read().tempo_range();
    let tracks = state.tracks.clone();
    blocking(move || tracks.refold_tempo_display(lo, hi)).await
}
