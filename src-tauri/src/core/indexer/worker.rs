//! The pool behind [`super::Indexer`]: a dispatcher that hands files to bounded
//! blocking workers, each of which parses one file and hands the row on.
//!
//! Workers never touch the connection pool. Everything they produce goes to the
//! single task in [`super::writer`], which is the only one that opens a
//! transaction.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::{IndexJob, Indexer};
use crate::core::hasher;
use crate::core::scanner::audio_extension;
use crate::error::Result;
use crate::models::{IndexStatus, TrackUpsert};

/// What a worker hands the writer. A re-scan of a 20,000-track library is
/// almost entirely `Touch`, and one single-row UPDATE per file would spend the
/// whole run in fsync — so the cheap case is batched exactly like the real one.
pub(super) enum WriteMsg {
    Row(TrackUpsert),
    Touch(String),
}

/// A progress event per file floods the webview at eight workers.
const EMIT_INTERVAL: Duration = Duration::from_millis(200);

pub(super) fn spawn_pool(indexer: Arc<Indexer>, jobs: UnboundedReceiver<IndexJob>) {
    let (rows_tx, rows_rx) = unbounded_channel::<WriteMsg>();
    // Long work started from `setup` goes on Tauri's managed runtime;
    // `tokio::spawn` panics with "no reactor running" at that point in startup.
    tauri::async_runtime::spawn(super::writer::writer_loop(indexer.clone(), rows_rx));
    tauri::async_runtime::spawn(dispatch_loop(indexer, jobs, rows_tx));
}

async fn dispatch_loop(
    indexer: Arc<Indexer>,
    mut jobs: UnboundedReceiver<IndexJob>,
    rows: UnboundedSender<WriteMsg>,
) {
    let mut emitter = tokio::time::interval(EMIT_INTERVAL);
    loop {
        let job = tokio::select! {
            job = jobs.recv() => match job {
                Some(job) => job,
                None => break,
            },
            _ = emitter.tick() => {
                if indexer.progress().is_scanning {
                    indexer.emit_progress();
                }
                continue;
            }
        };

        let counters = indexer.counters().clone();
        // Both choke points are cheap checks, and both have to be here: this
        // one stops work starting, the writer's stops work landing.
        if counters.cancelled.load(Ordering::Relaxed) || indexer.is_excluded(&job.path) {
            counters.skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        // A permit is held for the whole file, so the width of this stage is
        // the number of files open at once rather than the number of tasks.
        let Ok(permit) = indexer.permits().clone().acquire_owned().await else {
            break;
        };
        let indexer = indexer.clone();
        let rows = rows.clone();
        tauri::async_runtime::spawn(async move {
            process(indexer, job, rows).await;
            drop(permit);
        });
    }
}

/// One file, start to finish: read it off the disk and hand the row to the
/// writer. Failures are per-file on purpose — across twenty thousand tracks,
/// one unreadable file must cost one row and not the scan.
async fn process(
    indexer: Arc<Indexer>,
    job: IndexJob,
    rows: UnboundedSender<WriteMsg>,
) {
    let counters = indexer.counters().clone();
    counters.in_flight.fetch_add(1, Ordering::Relaxed);
    *counters.current_path.lock() = Some(job.path.to_string_lossy().into_owned());

    let tracks = indexer.tracks().clone();
    let path = job.path.clone();
    let probed = tauri::async_runtime::spawn_blocking(move || probe_one(&tracks, &path)).await;

    match probed {
        Ok(Ok(Outcome::Row(row))) => {
            let _ = rows.send(WriteMsg::Row(row));
        }
        Ok(Ok(Outcome::Unchanged(path))) => {
            counters.skipped.fetch_add(1, Ordering::Relaxed);
            // Still touched, or the reconciler marks it missing next launch.
            let _ = rows.send(WriteMsg::Touch(path));
        }
        Ok(Ok(Outcome::NotAudio)) => {
            counters.skipped.fetch_add(1, Ordering::Relaxed);
        }
        Ok(Err(e)) => fail(&indexer, &job.path, &e.to_string()),
        // A panic in a worker is one unreadable file, not a dead scan.
        Err(e) => fail(&indexer, &job.path, &e.to_string()),
    }
    counters.in_flight.fetch_sub(1, Ordering::Relaxed);
}

fn fail(indexer: &Arc<Indexer>, path: &Path, reason: &str) {
    record_failure(indexer, path, reason);
    indexer.counters().failed.fetch_add(1, Ordering::Relaxed);
}

/// What reading one file produced.
enum Outcome {
    Row(TrackUpsert),
    /// Already current. The row only needs its `last_seen_at` moved.
    Unchanged(String),
    NotAudio,
}

/// Reads one file, deciding first whether it needs reading at all — which is
/// what makes a re-scan cheap rather than a second full index.
fn probe_one(tracks: &Arc<crate::db::TrackRepository>, path: &Path) -> Result<Outcome> {
    let Some(extension) = audio_extension(path) else {
        return Ok(Outcome::NotAudio);
    };
    let meta = std::fs::metadata(path)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let key = path.to_string_lossy().into_owned();
    if !tracks.unchanged_since(&[(key.clone(), modified)])?.is_empty() {
        return Ok(Outcome::Unchanged(key));
    }

    // Header-only facts: cheap enough to run during the walk, so the table can
    // show duration and bitrate before anything is analysed.
    let probe = crate::analysis::decode::probe(path)?;
    // Tags and the content hash are best-effort. A file whose tag block is
    // malformed is still a track, and its row is more useful than its absence.
    let tags = crate::tags::read::read(path).ok();
    let content_hash = hasher::hash_audio_region(path).ok();

    Ok(Outcome::Row(TrackUpsert {
        id: hasher::hash_path(&path.to_string_lossy()),
        path: path.to_string_lossy().into_owned(),
        filename: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        extension,
        size_bytes: Some(meta.len() as i64),
        content_hash,
        tags,
        duration_ms: probe.duration_ms,
        sample_rate: Some(probe.sample_rate),
        channels: Some(probe.channels),
        bit_depth: probe.bit_depth,
        bitrate_kbps: probe.bitrate_kbps,
        codec: Some(probe.codec),
    }))
}

fn record_failure(indexer: &Arc<Indexer>, path: &Path, reason: &str) {
    tracing::debug!("indexing {} failed: {}", path.display(), reason);
    let id = hasher::hash_path(&path.to_string_lossy());
    // Only a row that already exists gets the error; a file that never indexed
    // has nothing to attach it to, and inventing a row for it would put an
    // unplayable entry in the table.
    let _ = indexer
        .tracks()
        .set_index_status(&id, IndexStatus::Error, Some(reason));
}
