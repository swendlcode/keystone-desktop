//! The whole pipeline over one real file, printed in full.
//!
//! This is the smoke test the two corpus paths cannot be: [`synthetic`] renders
//! its own audio and never opens a file, and [`giantsteps`] scores 600 of them
//! and prints one aggregate. Neither would notice a stage that returns a
//! plausible wrong answer on a real 320 kbit MP3 — a beat grid half a bar out,
//! eight cues clustered in the intro, an energy of 1 on a peak-time record.
//!
//! So it prints every field the engine produced rather than a verdict, and it
//! prints the detected tempo next to the folded one and the cue list in full,
//! because those are the two places a wrong answer still looks like a right
//! one.
//!
//! [`synthetic`]: super::synthetic
//! [`giantsteps`]: super::giantsteps

use std::fmt::Write as _;
use std::path::Path;

use crate::analysis::camelot::Camelot;
use crate::analysis::{self, tempo, AnalysisContext};
use crate::error::Result;
use crate::models::{AnalysisResult, Settings};

/// S1..S11 over one file, with the peaks cache off.
///
/// Peaks are a player cache keyed by content hash and this run has no library
/// to serve them to, so writing a pyramid into a temp directory would only
/// make the measurement slower than the app's.
pub fn run(path: &Path) -> Result<AnalysisResult> {
    let ctx = AnalysisContext {
        cache_peaks: false,
        ..AnalysisContext::new(
            std::env::temp_dir().join("keystone-eval-peaks"),
            Settings::default(),
        )
    };
    analysis::analyze_track(path, &ctx)
}

pub fn report(path: &Path, r: &AnalysisResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\n=== Keystone: one real file, S1..S11 ===\n");
    let _ = writeln!(out, "file       {}", path.display());
    let _ = writeln!(
        out,
        "duration   {}   analysis v{}, energy model v{}",
        clock(r.duration_ms as f64),
        r.analysis_version,
        r.energy_model_version
    );
    let _ = writeln!(out, "hash       {}", r.content_hash);
    let _ = write!(out, "{}", key_lines(r));
    let _ = write!(out, "{}", tempo_lines(r));
    let _ = write!(out, "{}", energy_lines(r));
    let _ = write!(out, "{}", cue_lines(r));
    out
}

fn key_lines(r: &AnalysisResult) -> String {
    let k = &r.key;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "key        {}  {}   confidence {:.2}{}",
        Camelot::from_key(k.root, k.mode).label(false),
        k.mode.as_str(),
        k.confidence,
        if k.mode_ambiguous {
            "   MODE AMBIGUOUS — the UI shows the relative pair, not a letter"
        } else {
            ""
        }
    );
    // The runner-up is what the ambiguity flag is about, so it is printed
    // whether or not the flag is set.
    if let (Some(root), Some(mode)) = (k.alt_root, k.alt_mode) {
        let _ = writeln!(
            out,
            "  runner-up {}  {}",
            Camelot::from_key(root, mode).label(false),
            mode.as_str()
        );
    }
    let _ = writeln!(
        out,
        "  tuning   {:+.1} cents   chunks {}",
        k.tuning_cents,
        k.chunk_keys.len()
    );
    out
}

fn tempo_lines(r: &AnalysisResult) -> String {
    let t = &r.tempo;
    let (lo, hi) = Settings::default().tempo_range();
    let mut out = String::new();
    // Raw beside folded, because folding the *chosen* tempo is what keeps a
    // correct 170 BPM and a wrong 85 BPM distinguishable in a report.
    let _ = writeln!(
        out,
        "tempo      {} BPM  (raw {:.3}, octave shift {:+})   confidence {:.2}",
        tempo::format_bpm(
            tempo::display_bpm(t.raw_bpm, lo, hi),
            Some(t.residual_ms),
            2
        ),
        t.raw_bpm,
        t.octave_shift,
        t.confidence
    );
    let _ = writeln!(
        out,
        "  grid     {} beats, residual {:.1} ms{}   two decimals {}",
        t.beats_ms.len(),
        t.residual_ms,
        if t.variable { ", VARIABLE TEMPO" } else { "" },
        if t.residual_ms < tempo::TWO_DECIMAL_RESIDUAL_MS {
            "earned"
        } else {
            "not earned"
        }
    );
    let _ = writeln!(
        out,
        "  downbeat phase {} of {}, confidence {:.2}   phrase {} bars",
        t.downbeat_phase, t.beats_per_bar, t.downbeat_confidence, t.phrase_bars
    );
    out
}

fn energy_lines(r: &AnalysisResult) -> String {
    let f = &r.energy.features;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "energy     {}   (raw {:.3})",
        r.energy.energy, r.energy.raw
    );
    let _ = writeln!(
        out,
        "  features onset/beat {:.2}  flux {:.3}  low {:.3}  high {:.3}  centroid {:.0}  \
         perc {:.3}  span {:.3}",
        f.onset_rate, f.flux_mean, f.low_ratio, f.high_ratio, f.centroid, f.perc_ratio,
        f.dynamic_span
    );
    let _ = writeln!(
        out,
        "levels     peak {:.1} dBFS, true peak {:.1}, {:.1} LUFS, {} clipped sample(s)",
        r.levels.peak_db, r.levels.true_peak_db, r.levels.lufs_integrated, r.levels.clipped_peaks
    );
    let _ = writeln!(out, "regions    {}", r.regions.len());
    for region in &r.regions {
        let _ = writeln!(
            out,
            "  {:<10} {:>8} -> {:>8}  {} bar(s)  energy {}",
            region.kind.as_str(),
            clock(region.start_ms),
            clock(region.end_ms),
            region.bars.unwrap_or(0),
            region.energy.map_or("—".into(), |e| e.to_string())
        );
    }
    out
}

fn cue_lines(r: &AnalysisResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "cues       {} of 8", r.cues.len());
    for cue in &r.cues {
        let _ = writeln!(
            out,
            "  CUE {}  {:>8}  {:<10} bar {:<5} phrase {:<3} confidence {:.2}",
            cue.slot + 1,
            clock(cue.position_ms),
            cue.kind.as_str(),
            cue.bar_index.map_or("—".into(), |b| b.to_string()),
            cue.phrase_level.map_or("—".into(), |p| p.to_string()),
            cue.confidence
        );
    }
    out
}

/// `m:ss.t`, which is how a cue position is read against a DJ app.
fn clock(ms: f64) -> String {
    if !ms.is_finite() || ms < 0.0 {
        return "—".to_string();
    }
    let total = ms / 1000.0;
    format!("{}:{:05.2}", (total / 60.0) as i64, total % 60.0)
}
