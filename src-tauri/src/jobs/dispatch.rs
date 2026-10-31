//! The queue's dispatcher: one thread that decides what runs next, and the
//! worker body it hands to the pool.
//!
//! Separate from [`super::queue`] so the public surface the commands call stays
//! readable next to the loop that drives it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::Emitter;

use crate::analysis;
use crate::analysis::version::ANALYSIS_VERSION;
use crate::core::JobPriority;
use crate::error::Result;
use crate::events::{self, QueueFinishedEvent};
use super::player;
use super::admit::admit;
use super::queue::{AnalysisJob, Shared, RESUME_CHUNK};
use super::writer::WriteMsg;
use crate::models::{IndexStatus, QueueState};

/// How long the dispatcher sleeps when there is nothing it can do. Every event
/// that creates work also notifies the condvar, so this is only a backstop
/// against a missed wake-up, not the latency of starting a job.
const IDLE_POLL: Duration = Duration::from_millis(200);

pub(crate) fn start(shared: Arc<Shared>) {
    thread::Builder::new()
        .name("keystone-analysis-dispatch".into())
        .spawn(move || run(shared))
        .expect("the analysis dispatcher thread could not be started");
}

fn run(shared: Arc<Shared>) {
    while !shared.shutdown.load(Ordering::Acquire) {
        match take_next(&shared) {
            Some(job) => spawn_job(&shared, job),
            None => idle_tick(&shared),
        }
    }
}

/// Pops one job, but only when there is a worker free for it. The width is read
/// every time so a settings change takes effect on the next job rather than at
/// the end of the run.
fn take_next(shared: &Arc<Shared>) -> Option<AnalysisJob> {
    if shared.paused.load(Ordering::Acquire) {
        return None;
    }
    let width = shared.width.load(Ordering::Acquire).max(1);
    if shared.inflight.load(Ordering::Acquire) >= width {
        return None;
    }
    shared.pending.lock().pop_front()
}

fn idle_tick(shared: &Arc<Shared>) {
    let empty = shared.pending.lock().is_empty();
    let quiet = empty && shared.inflight.load(Ordering::Acquire) == 0;
    if quiet && !shared.paused.load(Ordering::Acquire) {
        if shared.backlog_open.load(Ordering::Acquire) {
            if let Err(e) = refill(shared) {
                tracing::warn!("the resume query failed, closing the backlog: {}", e);
                shared.backlog_open.store(false, Ordering::Release);
            }
            return;
        }
        finish_run(shared);
    }
    let mut guard = shared.pending.lock();
    shared.wake.wait_for(&mut guard, IDLE_POLL);
}

/// Pulls the next chunk of unanalysed tracks.
///
/// This is the whole of resume-across-restarts: a track's `analysis_version`
/// advances only when its result has been committed, so an interrupted run
/// leaves exactly the right rows behind and this query finds them.
pub(crate) fn refill(shared: &Arc<Shared>) -> Result<usize> {
    let rows = shared
        .tracks
        .needing_analysis(ANALYSIS_VERSION, RESUME_CHUNK)?;
    let fresh: Vec<_> = rows
        .into_iter()
        .filter(|t| t.index_status != IndexStatus::Error)
        .filter(|t| !shared.attempted.lock().contains(&t.id))
        .collect();
    if fresh.is_empty() {
        // Either the library is analysed or everything left has already been
        // tried this session. Both mean stop asking.
        shared.backlog_open.store(false, Ordering::Release);
        return Ok(0);
    }
    admit(shared, fresh, JobPriority::Normal, false)
}

/// Fires once per run, when the queue has genuinely drained.
///
/// Distinct from progress reaching 100%: the sidebar's `Empty` is a state, and
/// a fraction that happens to read 412/412 is not it.
fn finish_run(shared: &Arc<Shared>) {
    let Some(started) = shared.run_started.lock().take() else {
        return;
    };
    shared.counters.set_state(QueueState::Idle);
    let snapshot = shared.counters.snapshot();
    shared.counters.emit(&shared.app);
    let _ = shared.app.emit(
        events::QUEUE_FINISHED,
        QueueFinishedEvent {
            // The two add up to everything committed, so the report reads
            // "1,200 analysed, 300 already known" rather than double-counting.
            analyzed: snapshot.done.saturating_sub(snapshot.from_cache),
            failed: snapshot.failed,
            from_cache: snapshot.from_cache,
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    );
}

fn spawn_job(shared: &Arc<Shared>, job: AnalysisJob) {
    shared.inflight.fetch_add(1, Ordering::AcqRel);
    shared.counters.begin(&job.path);
    shared.counters.maybe_emit(&shared.app);

    let shared = shared.clone();
    let pool = shared.pool.read().clone();
    pool.spawn(move || {
        // A panic in one stage must not wedge the queue: without this the slot
        // is never released and every later track waits on a file that already
        // failed.
        let outcome = catch_unwind(AssertUnwindSafe(|| analyze(&shared, &job)));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => report_failure(&shared, &job, e.to_string()),
            Err(_) => report_failure(&shared, &job, "the analysis panicked on this file".into()),
        }
        shared.counters.end();
        shared.inflight.fetch_sub(1, Ordering::AcqRel);
        shared.wake.notify_all();
    });
}

/// One track. Hands the result to the writer and never touches the pool: eight
/// workers competing for the single SQLite writer would spend the per-track
/// budget in busy retries.
fn analyze(shared: &Arc<Shared>, job: &AnalysisJob) -> Result<()> {
    let ctx = shared.ctx.read().clone();
    let result = analysis::analyze_track(Path::new(&job.path), &ctx)?;
    shared.writer.send(WriteMsg::Analyzed {
        track_id: job.track_id.clone(),
        result: Box::new(result),
        from_cache: false,
    });
    Ok(())
}

fn report_failure(shared: &Arc<Shared>, job: &AnalysisJob, message: String) {
    tracing::warn!("analysis failed for {}: {}", job.path, message);
    shared.writer.send(WriteMsg::Failed {
        track_id: job.track_id.clone(),
        message,
    });
}

/// A waveform decode, on the analysis pool but outside the analysis counters.
pub(crate) fn spawn_peaks(
    shared: &Arc<Shared>,
    track_id: String,
    path: String,
    content_hash: String,
) {
    shared.peaks_inflight.fetch_add(1, Ordering::AcqRel);
    let shared = shared.clone();
    let pool = shared.pool.read().clone();
    pool.spawn(move || {
        let dir = shared.ctx.read().peaks_dir.clone();
        let done = catch_unwind(AssertUnwindSafe(|| {
            player::compute_peaks(&dir, Path::new(&path), &content_hash)
        }));
        match done {
            Ok(Ok(_)) => player::emit_waveform_ready(&shared.app, &track_id, &content_hash),
            Ok(Err(e)) => tracing::warn!("peaks failed for {}: {}", path, e),
            Err(_) => tracing::warn!("peaks panicked for {}", path),
        }
        shared.peaks_inflight.fetch_sub(1, Ordering::AcqRel);
    });
}
