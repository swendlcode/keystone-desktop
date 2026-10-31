//! The single writer behind the index pool.
//!
//! Split from `worker.rs` because the two halves have opposite constraints:
//! the dispatcher wants as many blocking workers as the machine will give it,
//! and this wants exactly one task holding a transaction. WAL gives concurrent
//! readers but a single writer, so a second writer here buys only contention
//! and busy timeouts.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;

use super::worker::WriteMsg;
use super::Indexer;
use crate::db::track_repo::UPSERT_BATCH_SIZE;
use crate::error::Result;
use crate::models::TrackUpsert;

/// Long enough that a trickle of watcher events still batches, short enough
/// that the last few files of a scan do not sit unwritten.
const FLUSH_INTERVAL: Duration = Duration::from_millis(300);

/// The one task that opens a transaction, batching at [`UPSERT_BATCH_SIZE`].
pub(super) async fn writer_loop(indexer: Arc<Indexer>, mut rows: UnboundedReceiver<WriteMsg>) {
    let mut batch = Batch::default();
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    loop {
        tokio::select! {
            msg = rows.recv() => match msg {
                Some(WriteMsg::Row(row)) => {
                    batch.rows.push(row);
                    if batch.rows.len() >= UPSERT_BATCH_SIZE {
                        flush(&indexer, &mut batch).await;
                    }
                }
                Some(WriteMsg::Touch(path)) => {
                    batch.touches.push(path);
                    if batch.touches.len() >= UPSERT_BATCH_SIZE {
                        flush(&indexer, &mut batch).await;
                    }
                }
                None => {
                    flush(&indexer, &mut batch).await;
                    break;
                }
            },
            // The tail of a scan would otherwise sit in the buffer until the
            // next file arrived, which on a watcher event could be hours.
            _ = ticker.tick() => flush(&indexer, &mut batch).await,
        }
    }
}

#[derive(Default)]
struct Batch {
    rows: Vec<TrackUpsert>,
    touches: Vec<String>,
}

async fn flush(indexer: &Arc<Indexer>, batch: &mut Batch) {
    // The second choke point. These rows were parsed before the folder was
    // removed, and without this they land after the delete and resurrect it.
    let rows: Vec<TrackUpsert> = batch
        .rows
        .drain(..)
        .filter(|r| !indexer.is_excluded(Path::new(&r.path)))
        .collect();
    let touches: Vec<String> = batch
        .touches
        .drain(..)
        .filter(|p| !indexer.is_excluded(Path::new(p)))
        .collect();
    if rows.is_empty() && touches.is_empty() {
        return;
    }

    let counters = indexer.counters().clone();
    let tracks = indexer.tracks().clone();
    let n = rows.len();
    let written = tauri::async_runtime::spawn_blocking(move || {
        if !touches.is_empty() {
            // A failed touch costs one spurious `missing` at the next launch,
            // which the reconciler undoes. It must not fail the batch.
            if let Err(e) = tracks.touch_last_seen_by_paths(&touches) {
                tracing::debug!("touching {} unchanged path(s) failed: {}", touches.len(), e);
            }
        }
        if rows.is_empty() {
            return Ok(0);
        }
        commit_with_backoff(&tracks, &rows)
    })
    .await;
    // A touch-only batch moved no row the progress bar counts, so there is
    // nothing to add and nothing to report.
    if n == 0 {
        return;
    }
    record(&counters, n, written);
    indexer.emit_progress();
}

/// A batch either landed or it did not, and a panic counts as "did not".
///
/// Every file in a failed batch is counted failed rather than retried
/// individually: the retry already happened in [`commit_with_backoff`], and a
/// per-file fallback here would re-open the transaction the pool is contending
/// over in the first place.
fn record(
    counters: &Arc<super::Counters>,
    n: usize,
    // The outer error is the join handle's, the inner one the transaction's.
    written: std::result::Result<Result<usize>, tauri::Error>,
) {
    match written {
        Ok(Ok(_)) => {
            counters.done.fetch_add(n, Ordering::Relaxed);
            return;
        }
        Ok(Err(e)) => tracing::warn!("upsert batch of {} failed: {}", n, e),
        Err(e) => tracing::warn!("upsert batch of {} panicked: {}", n, e),
    }
    counters.failed.fetch_add(n, Ordering::Relaxed);
}

/// `busy_timeout` covers a reader that is mid-statement; it does not cover a
/// checkpoint that takes the write lock for longer than that. Three tries with
/// a growing wait turns a transient lock into a pause rather than a lost batch.
fn commit_with_backoff(
    tracks: &Arc<crate::db::TrackRepository>,
    batch: &[TrackUpsert],
) -> Result<usize> {
    let mut wait = Duration::from_millis(50);
    let mut last = None;
    for _ in 0..3 {
        match tracks.upsert_batch(batch) {
            Ok(n) => return Ok(n),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(wait);
                wait *= 4;
            }
        }
    }
    Err(last.expect("a failed retry loop recorded no error"))
}

