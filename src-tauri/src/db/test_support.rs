//! A real database on a real temp file for the repository tests.
//!
//! Not an in-memory connection: the pool hands out four of them and
//! `:memory:` would give each its own empty database, so a test would write
//! through one and read nothing back through another. This is also the only way
//! the WAL pragmas and the migration runner are exercised at all.

use std::sync::Arc;

use tempfile::TempDir;

use crate::db::{
    AnalysisRepository, CollectionRepository, CueRepository, DatabasePool, LibraryRepository,
    TrackRepository,
};
use crate::models::{TagSnapshot, TrackUpsert};

pub struct Harness {
    /// Held so the directory outlives the pool; dropping it deletes the file.
    _dir: TempDir,
    pub db: Arc<DatabasePool>,
    pub tracks: Arc<TrackRepository>,
    pub cues: Arc<CueRepository>,
    pub collections: Arc<CollectionRepository>,
    pub analysis: Arc<AnalysisRepository>,
    pub library: Arc<LibraryRepository>,
}

impl Harness {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("temp dir");
        let db = DatabasePool::open(&dir.path().join("keystone.db")).expect("open db");
        Self {
            _dir: dir,
            tracks: Arc::new(TrackRepository::new(db.clone())),
            cues: Arc::new(CueRepository::new(db.clone())),
            collections: Arc::new(CollectionRepository::new(db.clone())),
            analysis: Arc::new(AnalysisRepository::new(db.clone())),
            library: Arc::new(LibraryRepository::new(db.clone())),
            db,
        }
    }

    /// Inserts one track through the real upsert path, so the tests exercise the
    /// statement the walk actually uses rather than a hand-written INSERT.
    pub fn add(&self, path: &str) -> String {
        self.add_with(path, |_| {})
    }

    pub fn add_with(&self, path: &str, f: impl FnOnce(&mut TrackUpsert)) -> String {
        let filename = path.rsplit('/').next().unwrap_or(path).to_string();
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or("mp3")
            .to_lowercase();
        let mut row = TrackUpsert {
            id: crate::core::hasher::hash_path(path),
            path: path.to_string(),
            filename,
            extension,
            size_bytes: Some(1024),
            content_hash: None,
            tags: Some(TagSnapshot::default()),
            duration_ms: Some(300_000),
            sample_rate: Some(44_100),
            channels: Some(2),
            bit_depth: Some(16),
            bitrate_kbps: Some(320),
            codec: Some("mp3".into()),
        };
        f(&mut row);
        let id = row.id.clone();
        self.tracks.upsert_batch(&[row]).expect("upsert");
        id
    }

    /// Sets columns the upsert path deliberately refuses to write, so a test can
    /// stand up an analysed library without running the engine.
    pub fn set_analysis_columns(&self, id: &str, assignments: &str) {
        let conn = self.db.get().expect("conn");
        conn.execute(
            &format!("UPDATE tracks SET {assignments} WHERE id = ?1"),
            [id],
        )
        .expect("set analysis columns");
    }
}

/// An analysis result whose numbers are easy to recognise in an assertion.
pub fn analysis_result(
    content_hash: &str,
    root: u8,
    mode: crate::models::KeyMode,
    bpm: f64,
) -> crate::models::AnalysisResult {
    use crate::models::AnalysisResult;
    AnalysisResult {
        content_hash: content_hash.into(),
        analysis_version: 1,
        energy_model_version: 1,
        duration_ms: 300_000,
        key: stub_key(root, mode),
        tempo: stub_tempo(bpm),
        energy: stub_energy(),
        levels: crate::models::LevelsResult {
            peak_db: -0.3,
            true_peak_db: -0.1,
            lufs_integrated: -8.2,
            clipped_peaks: 0,
        },
        cues: Vec::new(),
        regions: Vec::new(),
        beatgrid: Vec::new(),
    }
}

fn stub_key(root: u8, mode: crate::models::KeyMode) -> crate::models::KeyResult {
    use crate::analysis::camelot::Camelot;
    use crate::models::{KeyMode, KeyResult};
    KeyResult {
        root,
        mode,
        camelot: Camelot::from_key(root, mode).label(false),
        confidence: 0.82,
        alt_root: Some(2),
        alt_mode: Some(KeyMode::Minor),
        mode_ambiguous: false,
        tuning_cents: -3.5,
        chunk_keys: Vec::new(),
    }
}

fn stub_tempo(bpm: f64) -> crate::models::TempoResult {
    crate::models::TempoResult {
        raw_bpm: bpm,
        octave_shift: 0,
        confidence: 0.9,
        residual_ms: 8.0,
        variable: false,
        beats_ms: Vec::new(),
        downbeat_phase: 0,
        downbeat_confidence: 0.7,
        beats_per_bar: 4,
        phrase_bars: 4,
    }
}

fn stub_energy() -> crate::models::EnergyResult {
    crate::models::EnergyResult {
        energy: 6,
        raw: 0.58,
        model_version: 1,
        features: crate::models::EnergyFeatures::default(),
    }
}

pub fn stub_analysis(content_hash: &str) -> crate::models::AnalysisResult {
    analysis_result(content_hash, 0, crate::models::KeyMode::Major, 128.0)
}
