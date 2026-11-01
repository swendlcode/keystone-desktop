use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, State};

use crate::commands::blocking;
use crate::core::{scanner, IndexJob, Indexer, JobPriority, ScannedFile, Scanner};
use crate::error::{KeystoneError, Result};
use crate::events;
use crate::models::{CleanCacheReport, LibraryFolder, ReconcileReport, ScanProgress, WatchedFolder};
use crate::state::AppState;

/// Lifts the indexer exclusion for this root, which matters because every
/// migration path here is remove-then-re-add: an exclusion left in place drops
/// the re-added folder's rows on the floor with no error.
#[tauri::command]
pub async fn add_watched_folder(path: String, state: State<'_, AppState>) -> Result<WatchedFolder> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(KeystoneError::NotFound(format!("{path} is not a folder")));
    }
    state.indexer.unexclude(&root);

    let library = state.library.clone();
    let upsert_path = path.clone();
    let folder = blocking(move || library.add_watched(&upsert_path)).await?;

    state.watcher.watch(&root)?;
    spawn_scan(state.indexer.clone(), root);
    Ok(folder)
}

/// Calls `Indexer::exclude` **before** deleting. A scan started earlier can
/// still have rows in flight that would otherwise land after the delete and
/// resurrect the folder.
#[tauri::command]
pub async fn remove_watched_folder(path: String, state: State<'_, AppState>) -> Result<usize> {
    let root = PathBuf::from(&path);
    state.indexer.exclude(&root);
    state.watcher.unwatch(&root)?;
    let library = state.library.clone();
    blocking(move || library.remove_watched(&path)).await
}

#[tauri::command]
pub async fn get_watched_folders(state: State<'_, AppState>) -> Result<Vec<WatchedFolder>> {
    let library = state.library.clone();
    blocking(move || library.list_watched()).await
}

/// Walks the root and hands every audio file to the IO stage. The CPU stage
/// picks the resulting rows up from its own resume query, so the two never need
/// to know about each other.
#[tauri::command]
pub async fn scan_folder(path: String, state: State<'_, AppState>) -> Result<usize> {
    let indexer = state.indexer.clone();
    blocking(move || {
        indexer.reset_counters();
        let (files, _stats) = Scanner::scan(&PathBuf::from(path))?;
        let jobs = to_jobs(files);
        let found = jobs.len();
        indexer.enqueue_batch(jobs);
        Ok(found)
    })
    .await
}

/// Takes whatever was dropped on the window: folders, loose audio files, or a
/// mix of both.
///
/// A dropped FOLDER becomes a watched root, exactly as the Add Tracks button
/// does — that is the normal way a library grows, and the user expects the
/// folder to keep tracking.
///
/// A dropped FILE is indexed on its own and its parent is NOT watched. Dropping
/// one track out of a Downloads folder holding four hundred must not pull in
/// the other three hundred and ninety-nine, which is what watching the parent
/// would do.
///
/// Returns how many files were handed to the indexer. Non-audio files are
/// skipped rather than refused: a drop is usually a selection, and failing the
/// whole gesture because one PDF came along with it is the wrong call.
#[tauri::command]
pub async fn add_dropped_paths(
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize> {
    let mut loose: Vec<IndexJob> = Vec::new();
    let mut folders: Vec<PathBuf> = Vec::new();

    for raw in paths {
        let path = PathBuf::from(&raw);
        if path.is_dir() {
            folders.push(path);
        } else if path.is_file() && scanner::audio_extension(&path).is_some() {
            loose.push(IndexJob {
                path,
                priority: JobPriority::Normal,
            });
        }
    }

    for root in &folders {
        state.indexer.unexclude(root);
        let library = state.library.clone();
        let as_string = root.to_string_lossy().to_string();
        blocking(move || library.add_watched(&as_string)).await?;
        state.watcher.watch(root)?;
    }

    let found = loose.len();
    if !loose.is_empty() {
        state.indexer.enqueue_batch(loose);
    }
    // Spawned after the loose files are queued so the single track a user
    // dropped to try the app out is not stuck behind a folder walk.
    for root in folders {
        spawn_scan(state.indexer.clone(), root);
    }

    Ok(found)
}

#[tauri::command]
pub async fn get_scan_progress(state: State<'_, AppState>) -> Result<ScanProgress> {
    Ok(state.indexer.progress())
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, AppState>) -> Result<()> {
    state.indexer.cancel();
    Ok(())
}

#[tauri::command]
pub async fn run_reconciliation(state: State<'_, AppState>) -> Result<ReconcileReport> {
    let library = state.library.clone();
    let reconciler = state.reconciler.clone();
    let report = blocking(move || {
        let roots: Vec<String> = library
            .list_watched()?
            .into_iter()
            .filter(|w| w.is_active)
            .map(|w| w.path)
            .collect();
        reconciler.run(&roots)
    })
    .await?;
    let _ = state.app.emit(events::RECONCILE_COMPLETE, report.clone());
    Ok(report)
}

/// Hard pass: drops rows marked missing, prunes the analysis cache and the peak
/// files nothing references. Behind an explicit action because an unmounted
/// external drive is the common cause of a missing row.
#[tauri::command]
pub async fn clean_cache(state: State<'_, AppState>) -> Result<CleanCacheReport> {
    let library = state.library.clone();
    let peaks_dir = state.peaks_dir.clone();
    blocking(move || library.clean_cache(&peaks_dir)).await
}

#[tauri::command]
pub async fn get_library_folders(state: State<'_, AppState>) -> Result<Vec<LibraryFolder>> {
    let library = state.library.clone();
    blocking(move || library.folders_with_counts()).await
}

/// The walk runs in the background, so ADD TRACKS returns as soon as the root is
/// recorded. A 20,000-track folder is minutes of walking and the rows appear in
/// the table as the IO stage commits them.
fn spawn_scan(indexer: Arc<Indexer>, root: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            indexer.reset_counters();
            match Scanner::scan(&root) {
                Ok((files, _stats)) => indexer.enqueue_batch(to_jobs(files)),
                Err(e) => tracing::warn!("the walk of {} failed: {}", root.display(), e),
            }
        })
        .await;
    });
}

fn to_jobs(files: Vec<ScannedFile>) -> Vec<IndexJob> {
    files
        .into_iter()
        .map(|f| IndexJob {
            path: f.path,
            priority: JobPriority::Normal,
        })
        .collect()
}
