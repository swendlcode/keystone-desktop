//! S1 to S11 over a file on disk, which is the one thing no other test does.
//!
//! The unit suite reaches every stage, and reaches S9, S10 and S11 through a
//! fixture built directly in the spectral domain. That fixture is the right
//! shape for what it tests — it enters the pipeline exactly where S9 does — but
//! it means the whole bottom of the engine, decode included, can be missing
//! while every number above it looks correct. That is how the engine came to
//! report a key accuracy on material it could not have read.
//!
//! So this file insists on the seam: synthesised audio, written out as a real
//! container, opened by symphonia, carried through `analyze_track`, and
//! asserted on as a whole `AnalysisResult`. What it checks is coherence rather
//! than accuracy — the tempo it was given, eight cues inside the file, an
//! energy on the stated scale, a hash to key the cache by. Accuracy is the eval
//! harness's job and it says so in its own report.
//!
//! Behind the `eval` feature with the rest of the slow suite: the pipeline runs
//! the HPSS median filters over the whole track, which is seconds of unoptimised
//! CPU and does not belong in a suite that has to stay quick enough to run on
//! every edit.

#![cfg(feature = "eval")]

use keystone_lib::analysis::decode::fixtures;
use keystone_lib::analysis::rhythm::fixtures as rhythm_fixtures;
use keystone_lib::analysis::spectral::test_signals as sig;
use keystone_lib::analysis::{analyze_track, AnalysisContext};
use keystone_lib::models::{AnalysisResult, Settings};

const SR: u32 = rhythm_fixtures::SAMPLE_RATE;
/// `test_signals::progression` lays its chords out on a 0.5 s beat, so the
/// harmony only lands on the bar lines at 120 BPM.
const BPM: f64 = 120.0;
/// The checkerboard kernel compares four bars against four, so S9 needs 16
/// strong beats — 32 beats, eight bars — before it can produce a boundary at
/// all. Twenty-four seconds is twelve bars at 120 BPM, which leaves room for
/// one.
const SECONDS: f64 = 24.0;

/// Four-to-the-floor under a minor progression, written out as a 16-bit WAV.
///
/// Both halves matter and for different stages: the clicks are what S6 and S7
/// track, and the chords are the only thing S2, S3 and S4 can read. A fixture
/// with only one of them exercises half the orchestration and passes.
fn render(dir: &std::path::Path) -> std::path::PathBuf {
    let click = rhythm_fixtures::click_track(BPM, SECONDS, rhythm_fixtures::Groove::FourToTheFloor);
    let chords = sig::progression(&sig::minor_iv(), 9, SR, SECONDS as f32, 0.0);
    let samples: Vec<f32> = click
        .samples
        .iter()
        .zip(chords.iter().chain(std::iter::repeat(&0.0)))
        .map(|(c, h)| (0.6 * c + 0.4 * h).clamp(-1.0, 1.0))
        .collect();
    let pcm = fixtures::Pcm {
        channels: 1,
        sample_rate: SR,
        samples,
    };
    fixtures::wav(dir, "pipeline.wav", &pcm, 16, false)
}

fn analyze(dir: &std::path::Path) -> AnalysisResult {
    let path = render(dir);
    let ctx = AnalysisContext {
        // Nothing reads the pyramid here and writing one would only make the
        // test slower.
        cache_peaks: false,
        ..AnalysisContext::new(dir.join("peaks"), Settings::default())
    };
    analyze_track(&path, &ctx).expect("the pipeline ran")
}

#[test]
fn the_whole_pipeline_runs_from_a_file_on_disk() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let r = analyze(dir.path());

    // The cache key. Empty here means a moved or re-tagged file re-analyses
    // from scratch, which is the one thing the data model is built to avoid.
    assert!(!r.content_hash.is_empty(), "no content hash");
    assert!(
        (r.duration_ms - (SECONDS * 1000.0) as i64).abs() < 200,
        "duration {} ms for a {SECONDS} s file",
        r.duration_ms
    );

    assert!(
        (r.tempo.raw_bpm - BPM).abs() < 1.0,
        "tempo {} for a {BPM} BPM click track",
        r.tempo.raw_bpm
    );
    assert!(!r.tempo.beats_ms.is_empty(), "no beat grid");
    assert_eq!(r.tempo.beats_per_bar, 4);
    // Two markers for a constant-tempo track, not one per beat.
    assert_eq!(r.beatgrid.len(), 2, "beatgrid {:?}", r.beatgrid);

    // S5 has to have run: a key with no Camelot label is a key the wheel
    // cannot filter on, which is the whole product.
    assert!(r.key.root < 12);
    assert!(!r.key.camelot.is_empty(), "no Camelot label");
    assert!((0.0..=1.0).contains(&r.key.confidence));
    assert!(
        r.key.tuning_cents.abs() < 10.0,
        "an in-tune fixture read as {:+.1} cents",
        r.key.tuning_cents
    );

    assert!(
        (1..=10).contains(&r.energy.energy),
        "energy {} is off the 1-10 scale",
        r.energy.energy
    );
    assert_eq!(r.energy_model_version, r.energy.model_version);
}

#[test]
fn every_cue_lands_inside_the_file_and_in_its_own_slot() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let r = analyze(dir.path());

    assert!(!r.cues.is_empty(), "no cue points");
    assert!(r.cues.len() <= 8, "{} cues for eight slots", r.cues.len());

    let mut slots: Vec<u8> = r.cues.iter().map(|c| c.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(slots.len(), r.cues.len(), "two cues share a slot");

    for cue in &r.cues {
        assert!(cue.slot < 8, "slot {} is outside CUE 1-8", cue.slot);
        assert!(
            cue.position_ms >= 0.0 && cue.position_ms <= r.duration_ms as f64,
            "{:?} at {} ms is outside a {} ms file",
            cue.kind,
            cue.position_ms,
            r.duration_ms
        );
    }

    // Sections are what the waveform colours and what the deck view would read.
    assert!(!r.regions.is_empty(), "no regions");
    for region in &r.regions {
        assert!(region.end_ms > region.start_ms, "region {region:?} is inverted");
        assert!(region.energy.is_some(), "region {region:?} carries no energy");
    }
}

/// Silence decodes, so it reaches the stages — and has no grid, so the stages
/// built on one have to refuse rather than invent a tempo and eight cues.
#[test]
fn a_file_with_no_grid_is_refused_rather_than_answered() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let pcm = fixtures::constant(0.0, SR as usize * 8, SR, 1);
    let path = fixtures::wav(dir.path(), "silence.wav", &pcm, 16, false);
    let ctx = AnalysisContext {
        cache_peaks: false,
        ..AnalysisContext::new(dir.path().join("peaks"), Settings::default())
    };

    let err = analyze_track(&path, &ctx).expect_err("silence has no grid");
    let message = err.to_string();
    assert!(
        message.contains("beat grid"),
        "the refusal has to say what was missing, not just fail: {message}"
    );
}

/// S12, which is the other half of "it works": a result that never reaches a
/// row is a 4 second decode the user cannot see.
///
/// The writer thread needs an `AppHandle` to emit on, so this drives the same
/// repository calls `jobs::writer::commit` makes, in the same order, and reads
/// every one of them back. What it is really guarding is the join: a field the
/// engine fills and the repository drops looks correct in both places on its
/// own.
#[test]
fn an_analysed_track_reaches_the_database_and_reads_back() {
    use keystone_lib::db::{AnalysisRepository, CueRepository, DatabasePool, TrackRepository};
    use keystone_lib::models::TrackUpsert;

    let dir = tempfile::tempdir().expect("a temp dir");
    let path = render(dir.path());
    let ctx = AnalysisContext {
        cache_peaks: false,
        ..AnalysisContext::new(dir.path().join("peaks"), Settings::default())
    };
    let result = analyze_track(&path, &ctx).expect("the pipeline ran");

    let db = DatabasePool::open(&dir.path().join("keystone.db")).expect("a database");
    let tracks = TrackRepository::new(db.clone());
    let cues = CueRepository::new(db.clone());
    let analysis = AnalysisRepository::new(db.clone());

    let id = keystone_lib::core::hasher::hash_path(&path.to_string_lossy());
    tracks
        .upsert_batch(&[TrackUpsert {
            id: id.clone(),
            path: path.to_string_lossy().into_owned(),
            filename: "pipeline.wav".into(),
            extension: "wav".into(),
            size_bytes: None,
            content_hash: Some(result.content_hash.clone()),
            tags: None,
            duration_ms: Some(result.duration_ms),
            sample_rate: Some(SR),
            channels: Some(1),
            bit_depth: Some(16),
            bitrate_kbps: None,
            codec: Some("pcm_s16le".into()),
        }])
        .expect("the track row");

    analysis.store(&result).expect("the cache row");
    cues
        .replace_detected(&id, &result.cues)
        .expect("the cue rows");
    cues
        .replace_beatgrid(&id, &result.beatgrid)
        .expect("the beatgrid rows");
    analysis
        .replace_regions(&id, &result.regions)
        .expect("the region rows");
    tracks
        .apply_analysis(&id, &result, Settings::default().tempo_range())
        .expect("the analysis columns");

    let row = tracks.get_by_id(&id).expect("read back").expect("a row");
    assert_eq!(row.camelot.as_deref(), Some(result.key.camelot.as_str()));
    assert_eq!(row.key_root, Some(result.key.root));
    assert_eq!(row.energy, Some(result.energy.energy));
    assert_eq!(row.energy_model_version, result.energy_model_version);
    assert_eq!(row.analysis_version, result.analysis_version);
    // Folded at read time from the raw value, which is what makes the Tempo
    // range setting free.
    let (lo, hi) = Settings::default().tempo_range();
    assert!(row.tempo_display.is_some_and(|d| d >= lo && d <= hi), "{:?}", row.tempo_display);

    assert_eq!(
        cues.list_for_track(&id).expect("the cues back").len(),
        result.cues.len()
    );
    assert_eq!(cues.beatgrid(&id).expect("the grid back").len(), 2);

    // The cache is the point of the content hash: this lookup is what makes a
    // moved, renamed or duplicated file cost no DSP at all.
    let cached = analysis
        .lookup(&result.content_hash, result.analysis_version)
        .expect("the cache lookup")
        .expect("a cached payload");
    assert_eq!(cached.key.camelot, result.key.camelot);
    assert_eq!(cached.cues.len(), result.cues.len());
}
