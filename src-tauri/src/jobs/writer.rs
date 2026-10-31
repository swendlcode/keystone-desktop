//! S12 — the single dedicated writer task.
//!
//! Analysis workers never touch the connection pool. They hand a finished
//! `AnalysisResult` to this thread, which is the only thing in the CPU stage
//! that writes. WAL gives concurrent readers but one writer, so eight workers
//! competing for it would spend the per-track budget in `SQLITE_BUSY` retries
//! while the table's own reads time out behind them.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::RwLock;

use tauri::{AppHandle, Emitter};

use crate::db::{AnalysisRepository, CueRepository, TrackRepository};
use crate::error::{KeystoneError, Result};
use crate::events::{self, TrackUpdatedEvent};
use super::progress::QueueCounters;
use crate::models::{AnalysisResult, IndexStatus, Settings};

/// How many results one drain pass commits before it goes back to the channel.
/// Per-result commits spend the budget in fsync; a much larger group holds the
/// single writer long enough that the table's reads start hitting the busy
/// timeout.
pub const WRITE_BATCH_SIZE: usize = 100;

/// Attempts per statement before a result is reported failed. The lock is held
/// by a reader, never forever, so a bounded backoff is enough.
const LOCK_RETRIES: u32 = 5;
const LOCK_BACKOFF_MS: u64 = 20;

pub enum WriteMsg {
    Analyzed {
        track_id: String,
        result: Box<AnalysisResult>,
        /// A content-hash cache hit: the rows are written but `analysis_cache`
        /// already holds the payload, so it is not stored again.
        from_cache: bool,
    },
    Failed {
        track_id: String,
        message: String,
    },
    Stop,
}

#[derive(Clone)]
pub struct WriterHandle {
    tx: Sender<WriteMsg>,
}

impl WriterHandle {
    pub fn send(&self, msg: WriteMsg) {
        // A closed channel means the writer thread is gone, which only happens
        // at shutdown. Dropping the result is correct there: the track keeps its
        // old `analysis_version` and the next launch re-queues it.
        let _ = self.tx.send(msg);
    }
}

pub struct WriterDeps {
    pub tracks: Arc<TrackRepository>,
    pub cues: Arc<CueRepository>,
    pub analysis: Arc<AnalysisRepository>,
    pub counters: Arc<QueueCounters>,
    pub app: AppHandle,
    /// Read for the tempo fold range only. The repository deliberately takes
    /// the range as an argument rather than knowing about `settings.json`, and
    /// the writer is the one place that holds both.
    pub settings: Arc<RwLock<Settings>>,
}

pub fn spawn(deps: WriterDeps) -> WriterHandle {
    let (tx, rx) = mpsc::channel::<WriteMsg>();
    thread::Builder::new()
        .name("keystone-analysis-writer".into())
        .spawn(move || run(rx, deps))
        .expect("the analysis writer thread could not be started");
    WriterHandle { tx }
}

fn run(rx: Receiver<WriteMsg>, deps: WriterDeps) {
    while let Ok(first) = rx.recv() {
        if matches!(first, WriteMsg::Stop) {
            return;
        }
        let mut batch = vec![first];
        // Take whatever else is already waiting, so a burst of eight workers
        // finishing together is one pass rather than eight.
        while batch.len() < WRITE_BATCH_SIZE {
            match rx.try_recv() {
                Ok(WriteMsg::Stop) => break,
                Ok(msg) => batch.push(msg),
                Err(_) => break,
            }
        }
        for msg in batch {
            handle(msg, &deps);
        }
        deps.counters.maybe_emit(&deps.app);
    }
}

fn handle(msg: WriteMsg, deps: &WriterDeps) {
    match msg {
        WriteMsg::Analyzed {
            track_id,
            result,
            from_cache,
        } => match commit(&track_id, &result, from_cache, deps) {
            Ok(()) => {
                deps.counters.committed(from_cache);
                emit_track(&track_id, deps);
            }
            Err(e) => {
                tracing::warn!("commit failed for {}: {}", track_id, e);
                record_failure(&track_id, &e.to_string(), deps);
            }
        },
        WriteMsg::Failed { track_id, message } => record_failure(&track_id, &message, deps),
        WriteMsg::Stop => {}
    }
}

/// One track's whole result, in the order a reader would want it: the row last,
/// so a crash between statements leaves the track still needing analysis rather
/// than claiming a version whose cues never landed.
fn commit(
    track_id: &str,
    result: &AnalysisResult,
    from_cache: bool,
    deps: &WriterDeps,
) -> Result<()> {
    if !from_cache {
        with_retry(|| deps.analysis.store(result))?;
    }
    with_retry(|| deps.cues.replace_detected(track_id, &result.cues).map(|_| ()))?;
    with_retry(|| deps.cues.replace_beatgrid(track_id, &result.beatgrid))?;
    with_retry(|| deps.analysis.replace_regions(track_id, &result.regions))?;
    let tempo_range = deps.settings.read().tempo_range();
    with_retry(|| deps.tracks.apply_analysis(track_id, result, tempo_range))?;
    with_retry(|| {
        deps.tracks
            .set_index_status(track_id, IndexStatus::Indexed, None)
    })
}

fn record_failure(track_id: &str, message: &str, deps: &WriterDeps) {
    deps.counters.failed();
    let status = with_retry(|| {
        deps.tracks
            .set_index_status(track_id, IndexStatus::Error, Some(message))
    });
    if let Err(e) = status {
        tracing::warn!("could not record the failure for {}: {}", track_id, e);
    }
    emit_track(track_id, deps);
}

/// The table patches the row in place from this, which is why it carries the
/// whole row rather than an id.
fn emit_track(track_id: &str, deps: &WriterDeps) {
    match deps.tracks.get_by_id(track_id) {
        Ok(Some(track)) => {
            let _ = deps
                .app
                .emit(events::TRACK_UPDATED, TrackUpdatedEvent { track });
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("could not read {} back after writing: {}", track_id, e),
    }
}

/// Retries only a busy or locked database. Anything else is a real error and
/// retrying it just delays the report.
fn with_retry<T>(mut op: impl FnMut() -> Result<T>) -> Result<T> {
    let mut attempt = 0;
    loop {
        match op() {
            Err(e) if attempt < LOCK_RETRIES && is_locked(&e) => {
                thread::sleep(Duration::from_millis(LOCK_BACKOFF_MS << attempt));
                attempt += 1;
            }
            other => return other,
        }
    }
}

fn is_locked(e: &KeystoneError) -> bool {
    use rusqlite::ErrorCode::{DatabaseBusy, DatabaseLocked};
    matches!(
        e,
        KeystoneError::Database(rusqlite::Error::SqliteFailure(inner, _))
            if matches!(inner.code, DatabaseBusy | DatabaseLocked)
    )
}
