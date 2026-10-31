//! How work enters the queue.
//!
//! Split from [`super::queue`] because the cache lookup is the interesting part
//! and it is shared: the command path and the resume refill must admit tracks
//! the same way, or a restart would do DSP that an `analysis_cache` hit already
//! paid for.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::analysis::version::ANALYSIS_VERSION;
use crate::core::JobPriority;
use crate::error::Result;
use crate::models::{IndexStatus, QueueState, Track};

use super::queue::{AnalysisJob, Shared};
use super::writer::WriteMsg;

/// Cache-lookup-then-queue, shared by the command path and the resume refill.
pub(crate) fn admit(
    shared: &Arc<Shared>,
    tracks: Vec<Track>,
    priority: JobPriority,
    force: bool,
) -> Result<usize> {
    let mut jobs = Vec::with_capacity(tracks.len());
    let mut hits = 0usize;
    for track in tracks {
        if track.index_status == IndexStatus::Missing {
            continue;
        }
        if !force && take_from_cache(shared, &track)? {
            hits += 1;
            continue;
        }
        shared.attempted.lock().insert(track.id.clone());
        jobs.push(AnalysisJob {
            track_id: track.id,
            path: track.path,
            force,
        });
    }
    let queued = jobs.len();
    push(shared, jobs, priority);
    if hits > 0 {
        shared.counters.emit(&shared.app);
    }
    Ok(queued + hits)
}

/// True when the result was already on hand and has been handed to the writer.
fn take_from_cache(shared: &Arc<Shared>, track: &Track) -> Result<bool> {
    let Some(hash) = track.content_hash.as_deref() else {
        return Ok(false);
    };
    let Some(cached) = shared.analysis.lookup(hash, ANALYSIS_VERSION)? else {
        return Ok(false);
    };
    shared.counters.cache_hit();
    shared.writer.send(WriteMsg::Analyzed {
        track_id: track.id.clone(),
        result: Box::new(cached),
        from_cache: true,
    });
    Ok(true)
}

fn push(shared: &Arc<Shared>, jobs: Vec<AnalysisJob>, priority: JobPriority) {
    if jobs.is_empty() {
        return;
    }
    let added = jobs.len();
    mark_running(shared);
    {
        let mut q = shared.pending.lock();
        for job in jobs {
            match priority {
                // A file somebody is waiting on goes in front of the backlog.
                JobPriority::High => q.push_front(job),
                JobPriority::Normal | JobPriority::Low => q.push_back(job),
            }
        }
    }
    shared.counters.add_queued(added);
    shared.wake.notify_all();
}

/// Starts a run's counters. Called before the first job is added so one run's
/// numbers never carry the last one's.
fn mark_running(shared: &Arc<Shared>) {
    let mut started = shared.run_started.lock();
    if started.is_none() {
        shared.counters.start_run();
        *started = Some(Instant::now());
    }
    if shared.paused.load(Ordering::Acquire) {
        shared.counters.set_state(QueueState::Paused);
    }
}

