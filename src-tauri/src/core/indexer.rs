//! The IO stage: walk, probe, read tags, hash, upsert. Bounded by a semaphore
//! because this stage is IO-bound and oversubscribing it only thrashes the disk.
//!
//! The CPU stage is a separate pool (`state::jobs`) so a 20,000-track library
//! is visible in the table long before it is analysed.

mod worker;
mod writer;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::AppHandle;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::Semaphore;

use crate::db::{DatabasePool, LibraryRepository, TrackRepository};
use crate::models::ScanProgress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    /// A file the user is waiting on, e.g. a single re-analysis from the
    /// context menu.
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone)]
pub struct IndexJob {
    pub path: PathBuf,
    pub priority: JobPriority,
}

impl IndexJob {
    pub fn normal(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            priority: JobPriority::Normal,
        }
    }
}

/// Counters and flags every worker shares. Separate from [`Indexer`] so a
/// worker task can hold it without holding the repositories.
#[derive(Default)]
pub(crate) struct Counters {
    pub total: AtomicUsize,
    pub done: AtomicUsize,
    pub skipped: AtomicUsize,
    pub failed: AtomicUsize,
    pub in_flight: AtomicUsize,
    pub cancelled: AtomicBool,
    pub current_path: Mutex<Option<String>>,
}

pub struct Indexer {
    tracks: Arc<TrackRepository>,
    /// Held for the folder-removal path, which is driven from the library
    /// commands rather than from inside the pool.
    #[allow(dead_code)]
    library: Arc<LibraryRepository>,
    app: AppHandle,
    jobs: UnboundedSender<IndexJob>,
    counters: Arc<Counters>,
    /// Roots whose work must not land. Checked at two choke points: before a
    /// job is processed, and in the upsert writer.
    excludes: Arc<Mutex<Vec<PathBuf>>>,
    permits: Arc<Semaphore>,
    /// How many permits the semaphore is meant to hold, so a shrink can take
    /// the difference back rather than guessing.
    width: Arc<Mutex<usize>>,
}

impl Indexer {
    pub fn spawn(
        db: Arc<DatabasePool>,
        tracks: Arc<TrackRepository>,
        library: Arc<LibraryRepository>,
        app: AppHandle,
        concurrency: usize,
    ) -> Arc<Self> {
        let _ = db;
        let width = concurrency.max(1);
        let (jobs, rx) = unbounded_channel::<IndexJob>();
        let me = Arc::new(Self {
            tracks,
            library,
            app,
            jobs,
            counters: Arc::new(Counters::default()),
            excludes: Arc::new(Mutex::new(Vec::new())),
            permits: Arc::new(Semaphore::new(width)),
            width: Arc::new(Mutex::new(width)),
        });
        worker::spawn_pool(me.clone(), rx);
        me
    }

    pub fn enqueue(&self, job: IndexJob) {
        if self.is_excluded(&job.path) {
            return;
        }
        self.counters.total.fetch_add(1, Ordering::Relaxed);
        // An unbounded channel on purpose: the walk produces faster than the
        // pool consumes, and back-pressure here would block the walk instead of
        // letting the table fill.
        let _ = self.jobs.send(job);
    }

    pub fn enqueue_batch(&self, jobs: Vec<IndexJob>) {
        let excludes = self.excludes.lock().clone();
        let accepted: Vec<IndexJob> = jobs
            .into_iter()
            .filter(|j| !excludes.iter().any(|root| j.path.starts_with(root)))
            .collect();
        self.counters
            .total
            .fetch_add(accepted.len(), Ordering::Relaxed);
        for job in accepted {
            let _ = self.jobs.send(job);
        }
    }

    pub fn progress(&self) -> ScanProgress {
        let c = &self.counters;
        let done = c.done.load(Ordering::Relaxed);
        let skipped = c.skipped.load(Ordering::Relaxed);
        let failed = c.failed.load(Ordering::Relaxed);
        let total = c.total.load(Ordering::Relaxed);
        ScanProgress {
            total,
            done,
            skipped,
            failed,
            current_path: c.current_path.lock().clone(),
            // Finished means nothing left to do, not `done == total`: a run can
            // end with skips and failures that never reach the done counter.
            is_scanning: done + skipped + failed < total
                || c.in_flight.load(Ordering::Relaxed) > 0,
        }
    }

    /// Resizes the semaphore live, without draining the queue. Growing adds
    /// permits; shrinking forgets them, which lets the workers already holding
    /// one finish rather than being cancelled mid-file.
    pub fn set_concurrency(&self, n: usize) {
        let n = n.max(1);
        let mut width = self.width.lock();
        if n > *width {
            self.permits.add_permits(n - *width);
        } else if n < *width {
            self.permits.forget_permits(*width - n);
        }
        *width = n;
    }

    pub fn reset_counters(&self) {
        let c = &self.counters;
        for counter in [&c.total, &c.done, &c.skipped, &c.failed] {
            counter.store(0, Ordering::Relaxed);
        }
        c.cancelled.store(false, Ordering::Relaxed);
        *c.current_path.lock() = None;
    }

    /// Stops this root's work from landing.
    ///
    /// `remove_watched_folder` must call this **before** deleting. A scan
    /// started earlier can still have hundreds of parsed rows in flight, and
    /// those writes would land after the delete and resurrect the folder.
    /// Exclusions are checked at two choke points — before a job is processed,
    /// and in the upsert writer, which is where already-parsed rows sit.
    pub fn exclude(&self, root: &Path) {
        let mut excludes = self.excludes.lock();
        if !excludes.iter().any(|p| p == root) {
            excludes.push(root.to_path_buf());
        }
    }

    /// Lifted when the folder is added again, which matters because every
    /// migration path here is remove-then-re-add.
    pub fn unexclude(&self, root: &Path) {
        self.excludes.lock().retain(|p| p != root);
    }

    pub(crate) fn is_excluded(&self, path: &Path) -> bool {
        self.excludes
            .lock()
            .iter()
            .any(|root| path.starts_with(root))
    }

    /// Drains the queue and lets in-flight jobs finish. Their rows still land:
    /// a cancelled scan leaves the files it had already read in the table,
    /// which is less surprising than losing them.
    pub fn cancel(&self) {
        self.counters.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn emit_progress(&self) {
        use tauri::Emitter;
        let _ = self.app.emit(
            crate::events::SCAN_PROGRESS,
            crate::events::ScanProgressEvent {
                progress: self.progress(),
            },
        );
    }

    pub(crate) fn counters(&self) -> &Arc<Counters> {
        &self.counters
    }

    pub(crate) fn tracks(&self) -> &Arc<TrackRepository> {
        &self.tracks
    }

    pub(crate) fn permits(&self) -> &Arc<Semaphore> {
        &self.permits
    }
}
