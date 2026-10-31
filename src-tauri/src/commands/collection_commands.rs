use tauri::State;

use crate::commands::blocking;
use crate::error::{KeystoneError, Result};
use crate::models::{Collection, CollectionKind};
use crate::state::AppState;

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>> {
    let collections = state.collections.clone();
    blocking(move || collections.list()).await
}

#[tauri::command]
pub async fn create_collection(name: String, state: State<'_, AppState>) -> Result<Collection> {
    let name = clean_name(name)?;
    let collections = state.collections.clone();
    blocking(move || collections.create(&name, CollectionKind::User, None)).await
}

#[tauri::command]
pub async fn rename_collection(id: String, name: String, state: State<'_, AppState>) -> Result<()> {
    let name = clean_name(name)?;
    let collections = state.collections.clone();
    blocking(move || collections.rename(&id, &name)).await
}

#[tauri::command]
pub async fn set_collection_color(
    id: String,
    color: Option<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    let collections = state.collections.clone();
    blocking(move || collections.set_color(&id, color.as_deref())).await
}

#[tauri::command]
pub async fn delete_collection(id: String, state: State<'_, AppState>) -> Result<()> {
    let collections = state.collections.clone();
    blocking(move || collections.delete(&id)).await
}

/// Takes the whole ordered list, so a drag across several rows is one atomic
/// write rather than a sequence the UI has to keep consistent.
#[tauri::command]
pub async fn reorder_collections(
    ordered_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    let collections = state.collections.clone();
    blocking(move || collections.reorder(&ordered_ids)).await
}

#[tauri::command]
pub async fn add_tracks_to_collection(
    collection_id: String,
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize> {
    let collections = state.collections.clone();
    blocking(move || collections.add_tracks(&collection_id, &track_ids)).await
}

#[tauri::command]
pub async fn remove_tracks_from_collection(
    collection_id: String,
    track_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize> {
    let collections = state.collections.clone();
    blocking(move || collections.remove_tracks(&collection_id, &track_ids)).await
}

#[tauri::command]
pub async fn reorder_collection_tracks(
    collection_id: String,
    ordered_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    let collections = state.collections.clone();
    blocking(move || collections.set_track_positions(&collection_id, &ordered_ids)).await
}

/// A sidebar row with a blank or whitespace-only name is unclickable and
/// unrenameable, so it is refused rather than created.
fn clean_name(name: String) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(KeystoneError::Other("a collection needs a name".into()));
    }
    Ok(trimmed.to_string())
}
