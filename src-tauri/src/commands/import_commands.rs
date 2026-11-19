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

/// A line from the webview, written into the app's own log and into a file.
///
/// The webview console is not the dev terminal, so a diagnostic the frontend
/// computes — why playback failed, what a probe of the asset URL returned —
/// is invisible to anyone reading the log that already carries the Rust side
/// of the same story. This puts both halves in one place.
///
/// The file half is not redundant. `tracing_subscriber::fmt()` writes to
/// stdout, and a bundle launched from Finder has no stdout anybody can read:
/// in the only build a user runs, every line sent here was being written to
/// nowhere. The file is also the half that survives the restart the user
/// reaches for when sound stops, which is what destroys the evidence.
///
/// Deliberately `warn`: nothing routine calls it.
#[tauri::command]
pub fn log_client(state: State<'_, AppState>, message: String) {
    tracing::warn!(target: "keystone_lib::client", "{message}");

    let path = state.data_dir.join("client-log.txt");
    // Capped rather than rotated. This is a file somebody is asked to send,
    // not an audit trail, and an unbounded append over a long session is a
    // diagnostic that fills a disk.
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len() > 256 * 1024)
        .unwrap_or(false);
    if too_big {
        let _ = std::fs::remove_file(&path);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // Every failure in here is a failure of a diagnostic write: it must never
    // interrupt the thing it is reporting on, so nothing is propagated.
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{stamp} {message}");
    }
}
