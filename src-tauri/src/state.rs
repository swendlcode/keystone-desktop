use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::analysis::AnalysisContext;
use crate::core::{FileWatcher, IndexJob, Indexer, JobPriority, Reconciler, WatchEvent};
use crate::db::{
    AnalysisRepository, CollectionRepository, CueRepository, DatabasePool, LibraryRepository,
    TrackRepository,
};
use crate::error::Result;
use crate::events::{self, TrackUpdatedEvent};
use crate::models::{IndexStatus, Settings, Track};

/// The analysis queue, its writer, the player's read paths and settings
/// persistence.
///
/// Declared here with an explicit path only because `lib.rs` is owned elsewhere
/// while the backend is built in parallel; the files sit at `src/jobs/` and the
/// declaration belongs beside the other top-level modules.
#[path = "jobs/mod.rs"]
pub mod jobs;

use jobs::AnalysisQueue;

pub struct AppState {
    pub app: AppHandle,
    pub db: Arc<DatabasePool>,
    pub tracks: Arc<TrackRepository>,
    pub cues: Arc<CueRepository>,
    pub collections: Arc<CollectionRepository>,
    pub analysis: Arc<AnalysisRepository>,
    pub library: Arc<LibraryRepository>,
    pub indexer: Arc<Indexer>,
    pub queue: Arc<AnalysisQueue>,
    pub reconciler: Arc<Reconciler>,
    pub watcher: Arc<FileWatcher>,
    pub settings: Arc<RwLock<Settings>>,
    pub settings_path: PathBuf,
    /// Waveform peak cache. Keyed by content hash, deliberately outside the
    /// database: the virtualized table selects a track row on every page, and a
    /// peak blob in that row costs a megabyte of SELECT per screen.
    pub peaks_dir: PathBuf,
    /// Dated copies of the DJ apps' collection files, taken before the first
    /// write to each target. The only thing that matters when it goes wrong.
    pub backups_dir: PathBuf,
}

struct Paths {
    data: PathBuf,
    settings: PathBuf,
    peaks: PathBuf,
    backups: PathBuf,
}

impl AppState {
    pub fn init(app: AppHandle) -> Result<Self> {
        let paths = resolve_paths(&app)?;
        let settings = Arc::new(RwLock::new(jobs::settings::load(&paths.settings)));
        let db = DatabasePool::open(&paths.data.join("keystone.db"))?;
        let repos = Repos::new(&db);
        let (indexer, queue) = spawn_pipeline(&db, &repos, &app, &paths, &settings);
        let reconciler = Arc::new(Reconciler::new(repos.tracks.clone(), repos.library.clone()));

        let (watch_tx, watch_rx) = unbounded_channel::<WatchEvent>();
        let watcher = Arc::new(FileWatcher::new(watch_tx)?);
        spawn_watch_bridge(
            indexer.clone(),
            repos.tracks.clone(),
            app.clone(),
            watch_rx,
        );

        Ok(Self {
            app,
            db,
            tracks: repos.tracks,
            cues: repos.cues,
            collections: repos.collections,
            analysis: repos.analysis,
            library: repos.library,
            indexer,
            queue,
            reconciler,
            watcher,
            settings,
            settings_path: paths.settings,
            peaks_dir: paths.peaks,
            backups_dir: paths.backups,
        })
    }

    pub fn save_settings(&self) -> Result<()> {
        let snapshot = self.settings.read().clone();
        jobs::settings::save(&self.settings_path, &snapshot)
    }

    /// Applies what a settings change implies, and nothing it does not.
    ///
    /// Everything here is in-memory. Returns the tempo range when, and only
    /// when, `tempo_display` has to be re-folded — the caller runs that on a
    /// blocking thread. Even then no file is read and no DSP runs: the three
    /// tempo columns are stored separately precisely so a user narrowing their
    /// range never pays for a re-analysis.
    pub fn apply_settings(&self, previous: &Settings) -> Option<(f64, f64)> {
        let next = self.settings.read().clone();
        self.indexer.set_concurrency(next.scanner_concurrency.max(1));
        self.queue
            .set_concurrency(next.effective_analysis_concurrency());
        let rebuilt = jobs::settings::rebuild_context(&self.queue.context(), &next);
        self.queue.set_context(Arc::new(rebuilt));
        jobs::settings::tempo_range_changed(previous, &next).then(|| next.tempo_range())
    }

    /// One row changed. The table patches in place from this rather than
    /// refetching the page, so every command that writes a track emits it.
    pub fn emit_track(&self, track: &Track) {
        let _ = self.app.emit(
            events::TRACK_UPDATED,
            TrackUpdatedEvent {
                track: track.clone(),
            },
        );
    }
}

/// The five repositories, built together so `init` does not spell out the same
/// clone five times.
struct Repos {
    tracks: Arc<TrackRepository>,
    cues: Arc<CueRepository>,
    collections: Arc<CollectionRepository>,
    analysis: Arc<AnalysisRepository>,
    library: Arc<LibraryRepository>,
}

impl Repos {
    fn new(db: &Arc<DatabasePool>) -> Self {
        Self {
            tracks: Arc::new(TrackRepository::new(db.clone())),
            cues: Arc::new(CueRepository::new(db.clone())),
            collections: Arc::new(CollectionRepository::new(db.clone())),
            analysis: Arc::new(AnalysisRepository::new(db.clone())),
            library: Arc::new(LibraryRepository::new(db.clone())),
        }
    }
}

/// The IO stage and the CPU stage, started with their own widths.
///
/// They are separate on purpose: the walk is IO-bound and finishes long before
/// the analysis does, which is what makes a 20,000-track library visible in the
/// table within seconds of being added.
fn spawn_pipeline(
    db: &Arc<DatabasePool>,
    repos: &Repos,
    app: &AppHandle,
    paths: &Paths,
    settings: &Arc<RwLock<Settings>>,
) -> (Arc<Indexer>, Arc<AnalysisQueue>) {
    let (scan_width, analysis_width, snapshot) = {
        let s = settings.read();
        (
            s.scanner_concurrency.max(1),
            s.effective_analysis_concurrency(),
            s.clone(),
        )
    };
    let indexer = Indexer::spawn(
        db.clone(),
        repos.tracks.clone(),
        repos.library.clone(),
        app.clone(),
        scan_width,
    );
    let ctx = Arc::new(AnalysisContext::new(paths.peaks.clone(), snapshot));
    let queue = AnalysisQueue::spawn(
        repos.tracks.clone(),
        repos.analysis.clone(),
        repos.cues.clone(),
        app.clone(),
        settings.clone(),
        ctx,
        analysis_width,
    );
    (indexer, queue)
}

fn resolve_paths(app: &AppHandle) -> Result<Paths> {
    let data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("keystone"));
    let peaks = data.join("peaks");
    let backups = data.join("backups");
    for dir in [&data, &peaks, &backups] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(Paths {
        settings: data.join("settings.json"),
        data,
        peaks,
        backups,
    })
}

/// Wires watcher events into the **IO** stage, never into the CPU stage
/// directly: a file save should put a row in the table immediately and then
/// join the analysis queue behind whatever is already in it.
fn spawn_watch_bridge(
    indexer: Arc<Indexer>,
    tracks: Arc<TrackRepository>,
    app: AppHandle,
    mut rx: UnboundedReceiver<WatchEvent>,
) {
    // Long-running work started from `setup` goes on Tauri's managed runtime.
    // `tokio::spawn` here panics with "no reactor running", because the Tokio
    // runtime is not live yet at this point in startup.
    tauri::async_runtime::spawn(async move {
        let _ = &app;
        while let Some(event) = rx.recv().await {
            let indexer = indexer.clone();
            let tracks = tracks.clone();
            // The database calls below are blocking, so they do not run on the
            // async runtime's thread.
            let _ = tauri::async_runtime::spawn_blocking(move || {
                handle_watch_event(&indexer, &tracks, event)
            })
            .await;
        }
    });
}

fn handle_watch_event(indexer: &Arc<Indexer>, tracks: &Arc<TrackRepository>, event: WatchEvent) {
    match event {
        WatchEvent::Created(path) | WatchEvent::Modified(path) => index(indexer, path),
        WatchEvent::Deleted(path) => mark_missing(tracks, &path),
        WatchEvent::Renamed { from, to } => {
            // Marked rather than moved: the new path is indexed from scratch and
            // its content hash finds the cached analysis, so a rename costs no
            // DSP either way.
            mark_missing(tracks, &from);
            index(indexer, to);
        }
    }
}

fn index(indexer: &Arc<Indexer>, path: PathBuf) {
    let is_audio = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| crate::core::scanner::is_audio(&e.to_lowercase()))
        .unwrap_or(false);
    if !is_audio {
        return;
    }
    indexer.enqueue(IndexJob {
        path,
        priority: JobPriority::Normal,
    });
}

/// Marked missing, never deleted. An unmounted external drive is the common
/// case and a DJ library on one is normal; deleting is behind Clean cache.
fn mark_missing(tracks: &Arc<TrackRepository>, path: &Path) {
    let found = tracks.get_by_path(&path.to_string_lossy());
    match found {
        Ok(Some(track)) => {
            if let Err(e) = tracks.set_index_status(&track.id, IndexStatus::Missing, None) {
                tracing::warn!("could not mark {} missing: {}", track.path, e);
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("could not look {} up: {}", path.display(), e),
    }
}
