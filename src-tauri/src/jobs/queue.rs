//! The CPU stage: a rayon pool, bounded by a user setting, behind our own
//! deque.
//!
//! **Resumable across restarts.** A first pass over 20,000 tracks is hours of
//! work, and an interrupted pass that starts from zero is unusable. Resume does
//! not need a persisted queue file: a track's `analysis_version` only advances
//! when its whole result has been committed, so "what still needs analysing" is
//! one indexed query and an interrupted run simply continues. The queue is a
//! sidebar object the user watches, which is why it also pauses, cancels and
//! reports.
//!
//! The work is held in a deque of our own rather than handed to rayon all at
//! once. Three things need that: the sidebar's queued count, cancel leaving
//! committed work alone, and resizing the pool without losing what is waiting.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::{Condvar, Mutex, RwLock};
use rayon::{ThreadPool, ThreadPoolBuilder};
use tauri::AppHandle;

use crate::analysis::AnalysisContext;
use crate::core::JobPriority;
use crate::db::{AnalysisRepository, CueRepository, TrackRepository};
use crate::error::Result;
use super::admit::admit;
use super::progress::QueueCounters;
use super::writer::{self, WriterDeps, WriterHandle};
use crate::models::{AnalysisProgress, QueueState, Settings, Track};

/// How many rows one resume query pulls. Enough that the pool never starves
/// between refills, small enough that a 160,000-track library does not build
/// one enormous deque at launch.
pub const RESUME_CHUNK: usize = 2_000;

/// Waveform decodes allowed alongside the queue. The player must be responsive
/// without being able to take the machine away from the run in progress.
pub const PEAKS_INFLIGHT_MAX: usize = 2;

#[derive(Debug, Clone)]
pub struct AnalysisJob {
    pub track_id: String,
    pub path: String,
    /// Skips the `analysis_cache` lookup. The context menu's Re-analyze Song.
    pub force: bool,
}

pub(crate) struct Shared {
    pub ctx: RwLock<Arc<AnalysisContext>>,
    pub pool: RwLock<Arc<ThreadPool>>,
    pub width: AtomicUsize,
    pub pending: Mutex<VecDeque<AnalysisJob>>,
    pub wake: Condvar,
    pub paused: AtomicBool,
    pub shutdown: AtomicBool,
    pub inflight: AtomicUsize,
    pub peaks_inflight: AtomicUsize,
    pub counters: Arc<QueueCounters>,
    pub writer: WriterHandle,
    pub app: AppHandle,
    pub tracks: Arc<TrackRepository>,
    pub analysis: Arc<AnalysisRepository>,
    /// Ids this session has already handed to a worker. Without it a file that
    /// fails every time is re-selected by every refill and the queue never
    /// drains.
    pub attempted: Mutex<HashSet<String>>,
    /// Set while the resume query still has rows to give.
    pub backlog_open: AtomicBool,
    pub run_started: Mutex<Option<Instant>>,
}

pub struct AnalysisQueue {
    shared: Arc<Shared>,
}

impl AnalysisQueue {
    pub fn spawn(
        tracks: Arc<TrackRepository>,
        analysis: Arc<AnalysisRepository>,
        cues: Arc<CueRepository>,
        app: AppHandle,
        settings: Arc<RwLock<Settings>>,
        ctx: Arc<AnalysisContext>,
        concurrency: usize,
    ) -> Arc<Self> {
        let counters = Arc::new(QueueCounters::default());
        let writer = writer::spawn(WriterDeps {
            tracks: tracks.clone(),
            cues,
            analysis: analysis.clone(),
            counters: counters.clone(),
            app: app.clone(),
            settings,
        });
        let width = concurrency.max(1);
        let shared = Arc::new(Shared {
            ctx: RwLock::new(ctx),
            pool: RwLock::new(build_pool(width)),
            width: AtomicUsize::new(width),
            pending: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            paused: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            inflight: AtomicUsize::new(0),
            peaks_inflight: AtomicUsize::new(0),
            counters,
            writer,
            app,
            tracks,
            analysis,
            attempted: Mutex::new(HashSet::new()),
            backlog_open: AtomicBool::new(false),
            run_started: Mutex::new(None),
        });
        super::dispatch::start(shared.clone());
        Arc::new(Self { shared })
    }

    /// Looks each track's content hash up in `analysis_cache` first.
    ///
    /// A moved file, a file Keystone itself renamed, and a duplicate all hit the
    /// cache with zero DSP — which is what makes a re-scan nearly free and the
    /// rename feature free instead of a re-analysis storm. Returns how many
    /// tracks were accounted for, cache hits included.
    pub fn enqueue(&self, track_ids: Vec<String>, priority: JobPriority) -> Result<usize> {
        let tracks = self.shared.tracks.get_many(&track_ids)?;
        admit(&self.shared, tracks, priority, false)
    }

    /// The context menu's Re-analyze Song: runs the DSP even on a cache hit.
    pub fn force_reanalyze(&self, track_ids: Vec<String>) -> Result<usize> {
        let tracks = self.shared.tracks.get_many(&track_ids)?;
        // High priority: this is the one case where somebody is watching one
        // specific row rather than a fraction in the sidebar.
        admit(&self.shared, tracks, JobPriority::High, true)
    }

    /// Called at startup, and whenever the user asks for everything unanalysed.
    /// The whole reason the queue survives a restart.
    pub fn enqueue_pending(&self) -> Result<usize> {
        self.shared.attempted.lock().clear();
        self.shared.backlog_open.store(true, Ordering::Release);
        let admitted = super::dispatch::refill(&self.shared)?;
        self.shared.wake.notify_all();
        Ok(admitted)
    }

    /// Decodes one file for waveform peaks, off the analysis counters.
    ///
    /// The sidebar's Analysis Queue is an analysis object. Clicking a row to see
    /// its waveform is not analysis, and inflating the queue by one track every
    /// time somebody browses would make the number meaningless. Returns false
    /// when the work was not scheduled, which the caller reports as "not cached
    /// yet" rather than an error.
    pub fn request_peaks(&self, track: &Track) -> bool {
        let Some(hash) = track.content_hash.clone() else {
            return false;
        };
        if self.shared.peaks_inflight.load(Ordering::Acquire) >= PEAKS_INFLIGHT_MAX {
            return false;
        }
        super::dispatch::spawn_peaks(&self.shared, track.id.clone(), track.path.clone(), hash);
        true
    }

    pub fn progress(&self) -> AnalysisProgress {
        self.shared.counters.snapshot()
    }

    /// Stops handing out work. Tracks already in flight finish and commit:
    /// throwing away two seconds of completed DSP to honour a pause is worse
    /// than the pause arriving a moment late.
    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
        self.shared.counters.set_state(QueueState::Paused);
        self.shared.counters.emit(&self.shared.app);
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Release);
        if !self.shared.counters.is_drained() {
            self.shared.counters.set_state(QueueState::Running);
        }
        self.shared.wake.notify_all();
        self.shared.counters.emit(&self.shared.app);
    }

    /// Drains what is waiting and closes the backlog. Completed work stays
    /// committed, and the cancelled tracks are still unanalysed, so the next
    /// launch picks them up again from the resume query.
    pub fn cancel(&self) {
        self.shared.backlog_open.store(false, Ordering::Release);
        let dropped = {
            let mut q = self.shared.pending.lock();
            let n = q.len();
            q.clear();
            n
        };
        self.shared.counters.drop_queued(dropped);
        self.shared.paused.store(false, Ordering::Release);
        self.shared.wake.notify_all();
        self.shared.counters.emit(&self.shared.app);
    }

    /// Resizes the pool without losing queued work: the queue is our own deque,
    /// so the old pool is simply dropped once its in-flight tasks finish.
    pub fn set_concurrency(&self, n: usize) {
        let n = n.max(1);
        if self.shared.width.swap(n, Ordering::AcqRel) == n {
            return;
        }
        *self.shared.pool.write() = build_pool(n);
        self.shared.wake.notify_all();
    }

    /// Swaps in a context rebuilt from changed settings. Jobs already running
    /// keep the context they started with, which is correct: a half-analysed
    /// track must not change its mind about whether it caches peaks.
    pub fn set_context(&self, ctx: Arc<AnalysisContext>) {
        *self.shared.ctx.write() = ctx;
    }

    pub fn context(&self) -> Arc<AnalysisContext> {
        self.shared.ctx.read().clone()
    }

    pub fn emit_progress(&self) {
        self.shared.counters.emit(&self.shared.app);
    }
}

fn build_pool(threads: usize) -> Arc<ThreadPool> {
    let built = ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("keystone-analysis-{i}"))
        .build();
    match built {
        Ok(pool) => Arc::new(pool),
        // A pool that will not build means the process cannot spawn threads at
        // all, which no fallback width fixes.
        Err(e) => panic!("the analysis pool could not be built: {e}"),
    }
}
