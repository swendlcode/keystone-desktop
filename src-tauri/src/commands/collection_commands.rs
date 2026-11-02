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

/// Writes a collection out as a CSV the user picks the destination for.
///
/// CSV rather than a real spreadsheet: Excel, Numbers and Sheets all open
/// it, it needs no dependency, and a tracklist pasted into a contract or a
/// radio submission wants the text more often than the formatting.
///
/// Built here rather than in the frontend because the frontend cannot write
/// to a path of the user's choosing — the fs plugin's default scope does not
/// reach outside the app's own directories, and widening it to arbitrary
/// paths to save one file would be a much larger permission than the feature
/// needs.
#[tauri::command]
pub async fn export_collection_csv(
    collection_id: String,
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<usize> {
    let tracks = state.tracks.clone();

    blocking(move || {
        let page = tracks.search(&crate::models::TrackQuery {
            filters: crate::models::TrackFilters {
                collection_id: Some(collection_id),
                ..crate::models::TrackFilters::default()
            },
            sort: crate::models::TrackSort::Artist,
            direction: crate::models::SortDirection::Asc,
            // A collection is a crate, not a library; one query is enough and
            // a cap keeps a mis-click from writing a hundred-megabyte file.
            limit: 20_000,
            offset: 0,
        })?;

        let mut csv = String::new();
        // A byte order mark, so Excel on Windows reads UTF-8 rather than the
        // system code page — without it every accented artist name arrives
        // mangled, which is most of a house tracklist.
        csv.push('\u{feff}');
        csv.push_str("#,Artist,Title,Key,BPM,Energy,Genre,Path\r\n");

        for (i, t) in page.tracks.iter().enumerate() {
            let row = [
                (i + 1).to_string(),
                t.artist.clone().unwrap_or_default(),
                t.title.clone().unwrap_or_else(|| t.filename.clone()),
                t.camelot.clone().unwrap_or_default(),
                t.tempo_display.map(|v| format!("{v:.2}")).unwrap_or_default(),
                t.energy.map(|v| v.to_string()).unwrap_or_default(),
                t.genre.clone().unwrap_or_default(),
                t.path.clone(),
            ];
            let cells: Vec<String> = row.iter().map(|c| csv_cell(c)).collect();
            csv.push_str(&cells.join(","));
            csv.push_str("\r\n");
        }

        std::fs::write(&dest_path, csv)?;
        Ok(page.tracks.len())
    })
    .await
}

/// RFC 4180: a cell containing a comma, a quote or a newline is quoted, and
/// its own quotes are doubled.
fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
