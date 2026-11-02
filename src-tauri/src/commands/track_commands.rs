use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

use crate::commands::blocking;
use crate::core::renamer;
use crate::db::TrackRepository;
use crate::error::{KeystoneError, Result};
use crate::models::{
    KeyMode, RenamePlan, RenamePlanItem, RenameReport, SortDirection, Track, TrackFilters,
    TrackPage, TrackQuery, TrackSort, ValueSource, WheelCounts,
};
use crate::state::AppState;

/// The table's one read path. Returns the page and the total together, because
/// the header shows the count beside the rows.
#[tauri::command]
pub async fn search_tracks(query: TrackQuery, state: State<'_, AppState>) -> Result<TrackPage> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.search(&query)).await
}

#[tauri::command]
pub async fn get_track(id: String, state: State<'_, AppState>) -> Result<Option<Track>> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.get_by_id(&id)).await
}

#[tauri::command]
pub async fn get_tracks(ids: Vec<String>, state: State<'_, AppState>) -> Result<Vec<Track>> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.get_many(&ids)).await
}

/// The genre options the library filter lists.
///
/// Derived from the library rather than from a fixed vocabulary: whatever the
/// files carry is what a DJ expects to filter on, and a curated list would both
/// offer genres nobody owns and hide ones they do.
#[tauri::command]
pub async fn get_genres(state: State<'_, AppState>) -> Result<Vec<String>> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.distinct_genres()).await
}

/// One count per wheel segment under the current filters, so a segment with
/// nothing behind it can read as empty rather than filtering to zero rows.
#[tauri::command]
pub async fn get_wheel_counts(
    filters: TrackFilters,
    state: State<'_, AppState>,
) -> Result<WheelCounts> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.wheel_counts(&filters)).await
}

#[tauri::command]
pub async fn set_rating(id: String, rating: Option<u8>, state: State<'_, AppState>) -> Result<Track> {
    if let Some(r) = rating.filter(|r| *r > 5) {
        return Err(KeystoneError::Other(format!("{r} is not a rating; the table shows five dots")));
    }
    write_then_read(&state, id, move |repo, id| repo.set_rating(id, rating)).await
}

/// A user override. Stamps `key_source = 'user'`, which is what makes it
/// survive the next analysis pass.
#[tauri::command]
pub async fn set_track_key(
    id: String,
    key_root: u8,
    key_mode: KeyMode,
    state: State<'_, AppState>,
) -> Result<Track> {
    if key_root > 11 {
        return Err(KeystoneError::Other(format!("{key_root} is not a pitch class")));
    }
    write_then_read(&state, id, move |repo, id| {
        repo.set_key(id, key_root, key_mode, ValueSource::User)
    })
    .await
}

#[tauri::command]
pub async fn set_track_tempo(id: String, bpm: f64, state: State<'_, AppState>) -> Result<Track> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(KeystoneError::Other(format!("{bpm} is not a tempo")));
    }
    write_then_read(&state, id, move |repo, id| {
        repo.set_tempo(id, bpm, ValueSource::User)
    })
    .await
}

#[tauri::command]
pub async fn set_track_energy(id: String, energy: u8, state: State<'_, AppState>) -> Result<Track> {
    if !(1..=10).contains(&energy) {
        return Err(KeystoneError::Other(format!("energy is 1-10, not {energy}")));
    }
    write_then_read(&state, id, move |repo, id| {
        repo.set_energy(id, energy, ValueSource::User)
    })
    .await
}

/// `delete_files` is false for the spec's Remove Song from Library. The ellipsis
/// in that menu item is the confirmation, and deleting the file is a separate,
/// louder decision — so the file goes first and the row only follows when it
/// succeeded, leaving a failure retryable rather than orphaned.
#[tauri::command]
pub async fn remove_tracks_from_library(
    ids: Vec<String>,
    delete_files: bool,
    state: State<'_, AppState>,
) -> Result<usize> {
    let tracks = state.tracks.clone();
    blocking(move || {
        let mut removed = 0;
        for track in tracks.get_many(&ids)? {
            if delete_files && Path::new(&track.path).exists() {
                if let Err(e) = std::fs::remove_file(&track.path) {
                    tracing::warn!("could not delete {}: {}", track.path, e);
                    continue;
                }
            }
            tracks.delete(&track.id)?;
            removed += 1;
        }
        Ok(removed)
    })
    .await
}

#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<()> {
    tauri_plugin_opener::reveal_item_in_dir(&path)
        .map_err(|e| KeystoneError::Other(format!("could not reveal {path}: {e}")))
}

/// The context menu's Export File: copies the audio file somewhere, with the
/// current tags already written.
///
/// Refuses rather than overwriting. The destination is somebody's folder and a
/// silent replace is not recoverable.
#[tauri::command]
pub async fn export_file(id: String, dest_dir: String, state: State<'_, AppState>) -> Result<String> {
    let tracks = state.tracks.clone();
    blocking(move || copy_track_to(&tracks, &id, &dest_dir)).await
}

/// Copy one track's file into a directory, refusing to overwrite.
///
/// Extracted so the MCP server's `export_files` copies exactly what the
/// window's Export Files does, including the refusal.
pub(crate) fn copy_track_to(
    tracks: &Arc<TrackRepository>,
    id: &str,
    dest_dir: &str,
) -> Result<String> {
    let track = tracks
        .get_by_id(id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("track {id}")))?;
    let dest = PathBuf::from(dest_dir).join(&track.filename);
    if dest.exists() {
        return Err(KeystoneError::Other(format!(
            "{} already holds a file called {}",
            dest_dir, track.filename
        )));
    }
    std::fs::copy(&track.path, &dest)?;
    Ok(dest.to_string_lossy().into_owned())
}

/// The preview is the undo. Renaming is the only irreversible operation in the
/// app, so nothing moves until the user has seen this.
#[tauri::command]
pub async fn preview_rename(ids: Vec<String>, state: State<'_, AppState>) -> Result<RenamePlan> {
    let tracks = state.tracks.clone();
    let settings = state.settings.read().clone();
    blocking(move || renamer::plan(&tracks.get_many(&ids)?, &settings)).await
}

#[tauri::command]
pub async fn rename_tracks(plan: RenamePlan, state: State<'_, AppState>) -> Result<RenameReport> {
    let tracks = state.tracks.clone();
    blocking(move || renamer::apply(&plan, &tracks)).await
}

/// Uses `original_filename` for an exact restore rather than a regex strip.
#[tauri::command]
pub async fn remove_key_from_filename(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<RenameReport> {
    let tracks = state.tracks.clone();
    blocking(move || {
        let items = tracks.get_many(&ids)?.iter().map(stripped_item).collect();
        renamer::apply(&RenamePlan { items }, &tracks)
    })
    .await
}

#[tauri::command]
pub async fn increment_play_count(id: String, state: State<'_, AppState>) -> Result<()> {
    let tracks = state.tracks.clone();
    blocking(move || tracks.increment_play_count(&id)).await
}

/// Collection contents, sorted like the main table. Takes the sort rather than
/// always using `position`, because a crate is browsed as well as ordered.
#[tauri::command]
pub async fn get_collection_page(
    collection_id: String,
    sort: TrackSort,
    direction: SortDirection,
    limit: usize,
    offset: usize,
    state: State<'_, AppState>,
) -> Result<TrackPage> {
    let tracks = state.tracks.clone();
    blocking(move || {
        tracks.search(&TrackQuery {
            filters: TrackFilters {
                collection_id: Some(collection_id),
                ..TrackFilters::default()
            },
            sort,
            direction,
            limit,
            offset,
        })
    })
    .await
}

/// Write one column, read the row back, tell the table.
///
/// The row goes back as a whole so the virtualized table can patch in place
/// instead of refetching the page the user is looking at.
async fn write_then_read<F>(state: &State<'_, AppState>, id: String, op: F) -> Result<Track>
where
    F: FnOnce(&Arc<TrackRepository>, &str) -> Result<()> + Send + 'static,
{
    let tracks = state.tracks.clone();
    let track = blocking(move || {
        op(&tracks, &id)?;
        tracks
            .get_by_id(&id)?
            .ok_or_else(|| KeystoneError::NotFound(format!("track {id}")))
    })
    .await?;
    state.emit_track(&track);
    Ok(track)
}

/// One item of the inverse rename. Skipped items carry their reason, because
/// the plan is what the user confirms.
fn stripped_item(track: &Track) -> RenamePlanItem {
    let path = Path::new(&track.path);
    let current_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| track.filename.clone());
    let stem = renamer::strip_stem(&current_stem, track.original_filename.as_deref());
    let filename = if track.extension.is_empty() {
        stem.clone()
    } else {
        format!("{stem}.{}", track.extension)
    };
    RenamePlanItem {
        track_id: track.id.clone(),
        from_path: track.path.clone(),
        to_path: path.with_file_name(&filename).to_string_lossy().into_owned(),
        skipped: (stem == current_stem).then(|| "its name carries no key to remove".to_string()),
    }
}
