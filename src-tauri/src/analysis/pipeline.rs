//! The orchestration: S1 to S11 for one track, and the only place the stage
//! order lives.
//!
//! Split out of [`super`] because that module is the engine's module tree and
//! its shared context, and this is a sequence — the two change for different
//! reasons and the file cap is there to keep them from sharing a reviewer's
//! attention.
//!
//! One decode feeds every stage. Decode is roughly 90% of the per-track cost on
//! paper, so the only optimisation that matters at this level is not decoding
//! twice; measured through this file the structure stage's HPSS filters are
//! actually the larger half, which is a note for whoever sizes the queue and
//! not a reason to decode twice.

use std::borrow::Cow;
use std::path::Path;

use super::{
    beats, cues, decode, energy, key, onset, rhythm, spectral, structure, tempo, version,
    AnalysisContext,
};
use crate::core::hasher;
use crate::core::peaks::{self, PeakPyramid};
use crate::error::{KeystoneError, Result};
use crate::models::{
    AnalysisResult, BeatgridMarker, DetectedCue, DetectedRegion, EnergyResult, KeyResult,
    TempoResult,
};

/// S1..S11 for one file, returning the whole `analysis_cache` payload.
///
/// In order, and the order is not rearrangeable: decode once (S1), tuning (S2),
/// chroma (S3), key (S4), Camelot (S5), onsets (S6), tempo and grid (S7),
/// downbeats and phrase period (S8), structure and regions (S9), energy (S10),
/// cues (S11). Persistence (S12) is the caller's — this function must not touch
/// the pool, because eight workers competing for the single SQLite writer would
/// spend the per-track budget in busy retries.
///
/// Never holds a decoded track longer than the stage that needs it — a
/// 10-minute 48 kHz stereo f32 buffer is 230 MB, and eight workers hold eight.
pub fn analyze_track(path: &Path, ctx: &AnalysisContext) -> Result<AnalysisResult> {
    // The audio-region hash, not a hash of the file: ID3 sits at the head of an
    // MP3, so hashing file bytes means Keystone's own re-tag pass changes every
    // hash and silently wipes the cache this payload is about to fill.
    let content_hash = hasher::hash_audio_region(path)?;
    let decode::DecodedAudio {
        mono_44100,
        mono_22050,
        duration_ms,
        levels,
        peaks: pyramid,
        ..
    } = decode::decode(path, &decode_options(ctx))?;
    cache_peaks(ctx, &content_hash, pyramid.as_ref(), path);

    let sr = decode::ANALYSIS_SAMPLE_RATE;
    // S2, first and not optional.
    let tuning = spectral::tuning::estimate(&mono_44100, sr)?;
    // S3, S4, S5. Scoped so the corrected copies and the 22.05 kHz buffer are
    // released before S9 materialises three spectrograms over the same track.
    let key = key_stage(&mono_44100, mono_22050, &tuning, ctx)?;
    // S6, S7, S8.
    let rhythm = rhythm_stage(&mono_44100, sr)?;
    // S9, S10, S11.
    let (energy, regions, cues) = shape_stage(&mono_44100, sr, &rhythm, duration_ms)?;

    Ok(AnalysisResult {
        content_hash,
        analysis_version: version::ANALYSIS_VERSION,
        energy_model_version: version::ENERGY_MODEL_VERSION,
        duration_ms,
        key,
        tempo: tempo_result(&rhythm),
        energy,
        levels,
        cues,
        regions,
        beatgrid: beatgrid(&rhythm),
    })
}

fn decode_options(ctx: &AnalysisContext) -> decode::DecodeOptions {
    decode::DecodeOptions {
        // The 22.05 kHz buffer is the second most expensive thing in a decode
        // and only the model voter reads it. No graph, no buffer.
        want_key_rate: ctx.skey.is_some(),
        want_peaks: ctx.cache_peaks,
        peak_zoom_levels: peaks::ZOOM_LEVELS.to_vec(),
    }
}

/// A failed peaks write must not fail the analysis: the pyramid is a cache
/// keyed by content hash and the player recomputes it on demand.
fn cache_peaks(ctx: &AnalysisContext, hash: &str, pyramid: Option<&PeakPyramid>, path: &Path) {
    let Some(pyramid) = pyramid else { return };
    if let Err(e) = peaks::write(&ctx.peaks_dir, hash, pyramid) {
        tracing::warn!("could not cache the waveform for {}: {}", path.display(), e);
    }
}

/// S3 -> S4 -> S5, with S2's answer already in hand.
///
/// Takes the 22.05 kHz buffer by value because this is the last stage that
/// reads it: returning it would keep a second copy of the track alive through
/// the structure stage, which is where the memory high-water mark is.
fn key_stage(
    mono: &[f32],
    mono_22050: Option<Vec<f32>>,
    est: &spectral::tuning::TuningEstimate,
    ctx: &AnalysisContext,
) -> Result<KeyResult> {
    let sr = decode::ANALYSIS_SAMPLE_RATE;
    // Two routes out of S2, and the residual is what tells S3 which one was
    // taken. Correcting against a residual of zero on an uncorrected buffer
    // leaves the detune in the chroma; the measurement itself still goes to the
    // UI, because reporting the residual would tell a vinyl rip it is at A440.
    let (tuned, residual) = if est.needs_correction {
        (
            Cow::Owned(spectral::tuning::correct(mono, sr, est.cents)?),
            0.0,
        )
    } else {
        (Cow::Borrowed(mono), est.cents)
    };
    let tuned_22050 = match mono_22050 {
        Some(buffer) if est.needs_correction => {
            spectral::tuning::correct(&buffer, decode::KEY_SAMPLE_RATE, est.cents)?
        }
        Some(buffer) => buffer,
        // Empty, not absent: the fusion treats a missing voter as missing
        // rather than as a vote for nothing.
        None => Vec::new(),
    };
    let pcp = spectral::chroma::pcp(&tuned, sr, residual)?;
    key::detect(
        &key::KeyInput {
            pcp: &pcp,
            mono: &tuned,
            sample_rate: sr,
            mono_22050: &tuned_22050,
            tuning_cents: est.cents,
            residual_cents: residual,
        },
        ctx.skey.as_deref(),
    )
}

/// S6 -> S7 -> S8, written out rather than through [`rhythm::analyze`] because
/// the per-beat chroma S8 reads can only be computed once S7 has produced the
/// beat times.
fn rhythm_stage(mono: &[f32], sr: u32) -> Result<rhythm::RhythmAnalysis> {
    let curves = onset::novelty(mono, sr)?;
    let (tempo, grid) = tempo::estimate(&curves)?;
    // From the *uncorrected* buffer deliberately. S8 reads this as a change
    // detector for the bass note under each candidate downbeat, so the detune
    // changes no answer it is asked for — while a tuning resample moves every
    // sample time by the ratio it moved the pitch, which would drift these rows
    // away from the beat times by seconds across a club edit.
    let beat_chroma = spectral::chroma::chroma_per_beat(mono, sr, &grid.beats_ms)?;
    // `None` is the classical phase. Through the arbiter seam rather than
    // `beats::analyze` so the one call site that would pass a neural downbeat
    // sequence is already here and visible; see `AnalysisContext::beat_arbiter`
    // for why there is nothing to pass.
    let beats = beats::analyze_with_arbiter(&grid, &curves, &beat_chroma, None)?;
    Ok(rhythm::RhythmAnalysis {
        curves,
        tempo,
        grid,
        beats,
    })
}

/// S9 -> S10 -> S11, which share one spectral pass over the whole track.
fn shape_stage(
    mono: &[f32],
    sr: u32,
    rhythm: &rhythm::RhythmAnalysis,
    duration_ms: i64,
) -> Result<(EnergyResult, Vec<DetectedRegion>, Vec<DetectedCue>)> {
    // Sections, energy and cues are all built on the grid, so a file with no
    // grid owes the user an explicit refusal rather than eight cue points
    // placed on an invented one.
    if rhythm.beats.downbeats_ms().len() < 2 {
        return Err(KeystoneError::Analysis(format!(
            "no usable beat grid ({} beats, tempo confidence {:.2}): sections, energy and \
             cue points all sit on it, so this file has a key and a tempo and nothing else",
            rhythm.beats.beats_ms.len(),
            rhythm.tempo.confidence
        )));
    }
    let short_term = energy::ShortTerm::measure(mono, sr)?;
    // The three spectrograms are the high-water mark of the whole pass — at
    // 4096/512 a five-minute track is ~200 MB each — so they live only as long
    // as the stage that reduces them to per-frame series.
    let mut structure = {
        let spectra = structure::spectra::Spectra::compute(mono, sr)?;
        structure::analyze(&spectra, &rhythm.beats, duration_ms)?
    };
    let mut regions = std::mem::take(&mut structure.regions);
    let (energy, core_ms) = energy::analyze(&structure, &short_term, &mut regions)?;
    let cues = cues::detect(
        &structure.novelty,
        &structure.boundaries,
        &structure.features,
        &structure.grid,
        core_ms,
    )?;
    Ok((energy, regions, cues))
}

/// The three tempo columns, unfolded. Folding is a read-time operation against
/// the user's range so the Tempo setting never costs a re-analysis.
fn tempo_result(r: &rhythm::RhythmAnalysis) -> TempoResult {
    TempoResult {
        raw_bpm: r.tempo.raw_bpm,
        octave_shift: r.tempo.octave_shift,
        confidence: r.tempo.confidence,
        residual_ms: r.tempo.residual_ms,
        variable: r.tempo.variable,
        beats_ms: r.grid.beats_ms.clone(),
        downbeat_phase: r.beats.downbeat_phase,
        downbeat_confidence: r.beats.downbeat_confidence,
        beats_per_bar: r.beats.beats_per_bar,
        phrase_bars: r.beats.phrase_bars,
    }
}

/// Two markers for a constant-tempo track, not one per beat.
///
/// The id and the track are persistence's to assign — `cue_repo` generates both
/// on insert — so the engine leaves them empty rather than minting ids for rows
/// that may never be written.
fn beatgrid(r: &rhythm::RhythmAnalysis) -> Vec<BeatgridMarker> {
    rhythm::beatgrid_markers(r)
        .into_iter()
        .map(|m| BeatgridMarker {
            id: String::new(),
            track_id: String::new(),
            position_ms: m.position_ms,
            bpm: m.bpm,
            beats_per_bar: m.beats_per_bar,
            beat_in_bar: m.beat_in_bar,
        })
        .collect()
}
