//! How far the analysis has got.
//!
//! Two sources, and the answer says which it used. The **queue** knows about
//! the run happening now — what is in flight, what failed, how fast it is
//! going — and only exists while the app is open. The **library** knows how
//! many rows carry an analysis at all, which is the question an agent is
//! usually really asking ("can I start mixing yet"), and is answerable with
//! the app closed.
//!
//! So the counts are always returned and the live half is added when there is
//! one. A tool that refused outright with the app shut would be refusing the
//! half it could have answered.

use serde_json::{json, Value};

use crate::error::Result;
use crate::mcp::McpContext;
use crate::models::{QueueState, TrackFilters, TrackQuery, TrackSort};

/// Below this many finished tracks the rate is not worth reporting: the first
/// file of a run carries the cost of warming the decoder and the thread pool,
/// so one sample is an estimate built on the least representative file in the
/// batch.
const MIN_SAMPLES_FOR_ETA: usize = 3;

pub fn get_status(ctx: &McpContext, _args: &Value) -> Result<Value> {
    let total = count(ctx, TrackFilters::default())?;
    let analyzed = count(
        ctx,
        TrackFilters {
            analyzed: Some(true),
            ..Default::default()
        },
    )?;

    let mut out = json!({
        "library": {
            "tracks": total,
            "analyzed": analyzed,
            "unanalyzed": total.saturating_sub(analyzed),
        },
    });

    let Some(live) = ctx.live.as_ref() else {
        out["running"] = json!(false);
        out["note"] = json!(
            "Keystone is not running, so nothing is being analysed right now; \
these are the stored counts. call launch_app to start a pass"
        );
        return Ok(out);
    };

    let p = live.queue.progress();
    let state = match p.state {
        QueueState::Idle => "idle",
        QueueState::Running => "running",
        QueueState::Paused => "paused",
    };
    out["running"] = json!(p.state == QueueState::Running);
    out["queue"] = json!({
        "state": state,
        "queued": p.queued,
        "inFlight": p.running,
        "done": p.done,
        "failed": p.failed,
        "fromCache": p.from_cache,
        "total": p.total,
        "currentFile": p.current_path,
    });

    if let Some(eta) = estimate(&p, live.queue.elapsed_ms()) {
        out["eta"] = eta;
    } else if p.state == QueueState::Running {
        out["eta"] = json!({
            "note": "too early to say; the rate is measured from this run and \
the first few files are the slowest",
        });
    }
    if p.state == QueueState::Idle && p.total > 0 {
        out["note"] = json!("the last pass has finished");
    }
    Ok(out)
}

/// Seconds remaining, from the rate **this run** has actually achieved.
///
/// Deliberately not derived from a per-track cost: `QueueCounters::snapshot`
/// refuses to put an `eta_ms` in the progress event for a good reason — every
/// timing behind such a number was measured on one machine, and Windows is
/// roughly two thirds of installs. This measures instead of assuming, which
/// is why it can be offered here: it is this computer, this library, these
/// settings, over the files already finished.
///
/// Cache hits are taken out of the rate. A pass over a library that has been
/// analysed before finishes hundreds of tracks a second and then slows to
/// real decoding the moment it reaches a new one, so leaving them in gives an
/// estimate that is wrong by two orders of magnitude at exactly the moment
/// somebody is waiting.
fn estimate(p: &crate::models::AnalysisProgress, elapsed_ms: Option<u64>) -> Option<Value> {
    let elapsed = elapsed_ms?;
    let remaining = p.queued + p.running;
    if p.state != QueueState::Running || remaining == 0 || elapsed == 0 {
        return None;
    }
    let computed = p.done.saturating_sub(p.from_cache);
    if computed < MIN_SAMPLES_FOR_ETA {
        return None;
    }
    let per_track_ms = elapsed as f64 / computed as f64;
    let seconds = (per_track_ms * remaining as f64 / 1000.0).round() as u64;
    Some(json!({
        "secondsRemaining": seconds,
        "tracksRemaining": remaining,
        "secondsPerTrack": (per_track_ms / 1000.0 * 100.0).round() / 100.0,
        "basedOn": format!("{computed} tracks analysed in this run on this machine"),
    }))
}

fn count(ctx: &McpContext, filters: TrackFilters) -> Result<usize> {
    Ok(ctx
        .tracks
        .search(&TrackQuery {
            filters,
            sort: TrackSort::Artist,
            direction: crate::models::SortDirection::Asc,
            limit: 1,
            offset: 0,
        })?
        .total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AnalysisProgress;

    fn running(done: usize, from_cache: usize, queued: usize) -> AnalysisProgress {
        AnalysisProgress {
            state: QueueState::Running,
            queued,
            running: 0,
            done,
            failed: 0,
            from_cache,
            total: done + queued,
            current_path: None,
            eta_ms: None,
        }
    }

    #[test]
    fn an_estimate_needs_a_few_real_analyses_behind_it() {
        assert!(estimate(&running(2, 0, 40), Some(10_000)).is_none());
        assert!(estimate(&running(3, 0, 40), Some(10_000)).is_some());
    }

    #[test]
    fn cache_hits_do_not_count_toward_the_rate() {
        // 21 done in 15s, but 18 of them were instant cache hits. The rate is
        // three real analyses in 15s — 5s each — so the 10 still queued are
        // 50 seconds away. Counting the cache hits would make it 0.71s each
        // and promise the whole remainder in seven.
        let e = estimate(&running(21, 18, 10), Some(15_000)).expect("an estimate");
        assert_eq!(e["secondsRemaining"], 50);
        assert_eq!(e["secondsPerTrack"], 5.0);
        assert_eq!(e["tracksRemaining"], 10);
    }

    #[test]
    fn a_queue_with_nothing_left_has_nothing_to_estimate() {
        assert!(estimate(&running(50, 0, 0), Some(60_000)).is_none());
    }

    #[test]
    fn an_idle_queue_is_not_given_a_finish_time() {
        let mut p = running(50, 0, 10);
        p.state = QueueState::Idle;
        assert!(estimate(&p, Some(60_000)).is_none());
    }

    /// Before the first run there is no clock, and a rate cannot be invented
    /// from one.
    #[test]
    fn no_run_clock_means_no_estimate() {
        assert!(estimate(&running(10, 0, 10), None).is_none());
    }
}
