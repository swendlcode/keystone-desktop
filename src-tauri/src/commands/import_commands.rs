//! Importing playlists: Apple Music's library XML, and `.m3u` / `.pls` files.

use std::path::{Path, PathBuf};

use tauri::State;

use crate::commands::blocking;
use crate::core::music_library::{self, MusicLibraryInfo};
use crate::core::{IndexJob, JobPriority};
use crate::error::Result;
use crate::state::AppState;

/// What the Music app is publishing, if anything.
///
/// `path` names an exported XML to read instead of the default location,
/// which is the way in for anyone who would rather export a library file than
/// leave XML sharing switched on.
///
/// Returns an empty `xmlPath` rather than an error when there is nothing to
/// read: the usual cause is that "Share Library XML with other applications"
/// is off, which is a setting the user can turn on, not a failure, and the
/// dialog says so.
#[tauri::command]
pub async fn get_music_library(path: Option<String>) -> Result<MusicLibraryInfo> {
    blocking(move || {
        let chosen = path.map(PathBuf::from).filter(|p| p.is_file());
        let Some(path) = chosen.or_else(music_library::library_xml_path) else {
            return Ok(MusicLibraryInfo {
                xml_path: None,
                playlists: Vec::new(),
            });
        };
        let playlists = music_library::read_playlists(&path)?;
        Ok(MusicLibraryInfo {
            xml_path: Some(path.to_string_lossy().into_owned()),
            playlists,
        })
    })
    .await
}

/// Queues every local file behind the named playlists.
///
/// `xmlPath` is passed back from the listing rather than looked up again, so
/// the import reads the same file the user chose from — and so a hand-picked
/// exported XML works as well as the one in the default location.
#[tauri::command]
pub async fn import_music_playlists(
    xml_path: String,
    names: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize> {
    let files =
        blocking(move || music_library::playlist_files(Path::new(&xml_path), &names)).await?;
    Ok(enqueue(&state, files))
}

/// Queues every file listed in an `.m3u`, `.m3u8` or `.pls`.
#[tauri::command]
pub async fn import_playlist_file(path: String, state: State<'_, AppState>) -> Result<usize> {
    let files = blocking(move || music_library::read_playlist_file(Path::new(&path))).await?;
    Ok(enqueue(&state, files))
}

/// Hands files to the indexer without watching anything.
///
/// The same rule a dropped loose file follows, and for a stronger reason: a
/// playlist's tracks are scattered across the disk, so watching their parents
/// would quietly adopt every unrelated folder they happen to live in — on a
/// normal Music library, the whole of `~/Music`.
fn enqueue(state: &State<'_, AppState>, files: Vec<PathBuf>) -> usize {
    if files.is_empty() {
        return 0;
    }
    let count = files.len();
    state.indexer.enqueue_batch(
        files
            .into_iter()
            .map(|path| IndexJob {
                path,
                priority: JobPriority::Normal,
            })
            .collect(),
    );
    count
}
