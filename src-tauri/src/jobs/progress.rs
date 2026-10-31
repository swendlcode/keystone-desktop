//! The analysis queue's counters, and the one place that decides how often the
//! webview hears about them.
//!
//! Progress is emitted on a timer rather than per track. At eight workers a
//! per-track emit floods the webview, and the sidebar only ever renders a
//! fraction and a filename.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::events::{self, AnalysisProgressEvent};
use crate::models::{AnalysisProgress, QueueState};

/// How often the sidebar hears about progress while the queue runs.
const EMIT_INTERVAL_MS: u64 = 250;

const STATE_IDLE: usize = 0;
const STATE_RUNNING: usize = 1;
const STATE_PAUSED: usize = 2;

/// Shared, lock-free except for the current path, which is a string and is read
/// far less often than it is written.
#[derive(Default)]
pub struct QueueCounters {
    state: AtomicUsize,
    queued: AtomicUsize,
    running: AtomicUsize,
    done: AtomicUsize,
    failed: AtomicUsize,
    from_cache: AtomicUsize,
    total: AtomicUsize,
    current_path: Mutex<Option<String>>,
    last_emit_ms: AtomicU64,
}

impl QueueCounters {
    /// Zeroes everything but leaves the state alone. Called when the queue goes
    /// from idle to running, so one run's numbers never include the last one's.
    pub fn start_run(&self) {
        for c in [
            &self.queued,
            &self.running,
            &self.done,
            &self.failed,
            &self.from_cache,
            &self.total,
        ] {
            c.store(0, Ordering::Relaxed);
        }
        *self.current_path.lock() = None;
        self.state.store(STATE_RUNNING, Ordering::Relaxed);
    }

    pub fn set_state(&self, state: QueueState) {
        let v = match state {
            QueueState::Idle => STATE_IDLE,
            QueueState::Running => STATE_RUNNING,
            QueueState::Paused => STATE_PAUSED,
        };
        self.state.store(v, Ordering::Relaxed);
    }

    pub fn state(&self) -> QueueState {
        match self.state.load(Ordering::Relaxed) {
            STATE_RUNNING => QueueState::Running,
            STATE_PAUSED => QueueState::Paused,
            _ => QueueState::Idle,
        }
    }

    pub fn add_queued(&self, n: usize) {
        self.queued.fetch_add(n, Ordering::Relaxed);
        self.total.fetch_add(n, Ordering::Relaxed);
    }

    /// A track left the queue and a worker picked it up.
    pub fn begin(&self, path: &str) {
        decrement(&self.queued);
        self.running.fetch_add(1, Ordering::Relaxed);
        *self.current_path.lock() = Some(path.to_string());
    }

    /// The DSP finished, successfully or not. Separate from [`Self::committed`]
    /// because `done` means written, not computed.
    pub fn end(&self) {
        decrement(&self.running);
    }

    pub fn committed(&self, from_cache: bool) {
        self.done.fetch_add(1, Ordering::Relaxed);
        if from_cache {
            self.from_cache.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }

    /// A cache hit never reaches a worker, so it is queued and committed in one
    /// step. This is the answer to "why was the re-scan instant".
    pub fn cache_hit(&self) {
        self.total.fetch_add(1, Ordering::Relaxed);
        self.done.fetch_add(1, Ordering::Relaxed);
        self.from_cache.fetch_add(1, Ordering::Relaxed);
    }

    /// Cancel drops what is still queued. The total shrinks to what was already
    /// done plus what is still in flight, so the fraction cannot read as a run
    /// that stalled at 40%.
    pub fn drop_queued(&self, n: usize) {
        for _ in 0..n {
            decrement(&self.queued);
            decrement(&self.total);
        }
    }

    pub fn snapshot(&self) -> AnalysisProgress {
        AnalysisProgress {
            state: self.state(),
            queued: self.queued.load(Ordering::Relaxed),
            running: self.running.load(Ordering::Relaxed),
            done: self.done.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            from_cache: self.from_cache.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            current_path: self.current_path.lock().clone(),
            // Deliberately absent. Every timing behind an estimate was measured
            // on an M2 Max, Windows is roughly two thirds of installs, and a
            // wrong "time remaining" is worse than none.
            eta_ms: None,
        }
    }

    pub fn is_drained(&self) -> bool {
        self.queued.load(Ordering::Relaxed) == 0 && self.running.load(Ordering::Relaxed) == 0
    }

    /// Emits at most once per [`EMIT_INTERVAL_MS`]. Safe to call from a worker.
    pub fn maybe_emit(&self, app: &AppHandle) {
        let now = now_ms();
        let last = self.last_emit_ms.load(Ordering::Relaxed);
        if now.saturating_sub(last) < EMIT_INTERVAL_MS {
            return;
        }
        if self
            .last_emit_ms
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        self.emit(app);
    }

    /// Unconditional. Used at the edges of a run, where the sidebar switching
    /// to or from `Empty` must not wait for the next tick.
    pub fn emit(&self, app: &AppHandle) {
        self.last_emit_ms.store(now_ms(), Ordering::Relaxed);
        let payload = AnalysisProgressEvent {
            progress: self.snapshot(),
        };
        let _ = app.emit(events::ANALYSIS_PROGRESS, payload);
    }
}

/// Saturating, because a counter that underflows to `usize::MAX` renders as a
/// queue of eighteen quintillion tracks.
fn decrement(c: &AtomicUsize) {
    let _ = c.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(v.saturating_sub(1))
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
