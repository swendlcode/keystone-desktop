//! One TSV row per file of the raw evidence the fuser sees.
//!
//! The confidence gates in [`fuse`] were fitted on synthetic material and the
//! 2026-10-31 run showed real records sitting below them, so the next fit has to
//! be made against the distribution of the gate input on real audio rather than
//! against another guess. This prints that input: the template voter's spread,
//! the duration support, the top two fused probabilities the separation term
//! reads, and the chunk agreement — beside the answer, so the table can be
//! joined to a reference label and the gates chosen from it.
//!
//! It is deliberately **not** a scorer. It emits raw numbers and no verdict;
//! `mirex.rs` stays the only thing in the tree that decides whether an estimate
//! was right, and the fit and the report are computed outside from this table.
//!
//! [`fuse`]: crate::analysis::key::fuse

use std::path::Path;

use crate::analysis::decode::{self, DecodeOptions};
use crate::analysis::key::skey::SkeySession;
use crate::analysis::{camelot::Camelot, key::KeyDetection};
use crate::error::Result;

use super::key_path;

/// Tab-separated so a path containing a comma, a quote or a Cyrillic title
/// needs no escaping; the path is last for the same reason.
pub const HEADER: &str = "seconds\tspread\twhole_spread\tchunk_p25\tchunk_med\tchunk_p75\t\
chunk_max\tsupport\tevidence\ttop_p\tsecond_p\tseparation\t\
agreement\trelative_conf\tparallel_conf\twhole_track_fallback\tchunks\tconfidence\t\
mode_ambiguous\tcamelot\troot\tmode\talt_camelot\ttuning_cents\tpath";

/// Decode and run S2 -> S3 -> S4 over one file, then format the row.
///
/// The same [`key_path::run`] the scorer uses, so a row in this table and a
/// score from the harness describe the same answer. Peaks and the model's rate
/// are not asked for: nothing here reads them and they are most of the cost.
pub fn row(path: &Path, skey: Option<&SkeySession>) -> Result<String> {
    let decoded = decode::decode(
        path,
        &DecodeOptions {
            want_key_rate: false,
            want_peaks: false,
            peak_zoom_levels: Vec::new(),
        },
    )?;
    let sr = decode::ANALYSIS_SAMPLE_RATE;
    let seconds = decoded.mono_44100.len() as f32 / sr as f32;
    let run = key_path::run(&decoded.mono_44100, sr, skey)?;
    Ok(format(path, seconds, &run.detection))
}

/// Linear-interpolated quantile of an already-sorted slice; 0 for an empty one,
/// which is a track too short to chunk rather than a missing measurement.
fn quantile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = p * (sorted.len() - 1) as f32;
    let lo = i.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (i - lo as f32)
}

fn format(path: &Path, seconds: f32, d: &KeyDetection) -> String {
    let (_, top) = d.fused.best();
    let (_, second) = d.fused.runner_up();
    let separation = if top + second > 0.0 {
        (top - second) / (top + second)
    } else {
        0.0
    };
    let mut cs = d.chunk_spreads.clone();
    cs.sort_by(f32::total_cmp);
    let k = &d.result;
    let alt = match (k.alt_root, k.alt_mode) {
        (Some(r), Some(m)) => Camelot::from_key(r, m).label(false),
        _ => "-".to_string(),
    };
    format!(
        "{seconds:.3}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t\
{top:.6}\t{second:.6}\t{separation:.6}\t\
{:.6}\t{:.6}\t{:.6}\t{}\t{}\t{:.6}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}",
        d.spread,
        d.whole_track_spread,
        quantile(&cs, 0.25),
        quantile(&cs, 0.50),
        quantile(&cs, 0.75),
        cs.last().copied().unwrap_or(0.0),
        d.duration_support,
        d.evidence,
        d.chunk_agreement,
        d.relative_confidence,
        d.parallel_confidence,
        u8::from(d.whole_track_fallback),
        k.chunk_keys.len(),
        k.confidence,
        u8::from(k.mode_ambiguous),
        k.camelot,
        k.root,
        k.mode.as_str(),
        alt,
        k.tuning_cents,
        path.display(),
    )
}

/// Every path on stdin, one row each, `threads` at a time.
///
/// A file that will not decode prints a `#` comment carrying the reason rather
/// than vanishing: a table that silently shrinks is how a calibration run ends
/// up fitted on the files that happened to be easy.
pub fn run(paths: &[std::path::PathBuf], threads: usize) -> Vec<String> {
    use rayon::prelude::*;
    let work = || {
        paths
            .par_iter()
            .map(|p| match row(p, None) {
                Ok(line) => line,
                Err(e) => format!("# unreadable\t{}\t{}", e, p.display()),
            })
            .collect::<Vec<_>>()
    };
    match rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
    {
        Ok(pool) => pool.install(work),
        Err(_) => paths.iter().map(|p| match row(p, None) {
            Ok(line) => line,
            Err(e) => format!("# unreadable\t{}\t{}", e, p.display()),
        }).collect(),
    }
}
