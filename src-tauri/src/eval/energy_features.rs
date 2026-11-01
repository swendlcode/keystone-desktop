//! The seven energy features over a list of real files, one row each.
//!
//! **Dev-only, behind the `eval` feature.** This is the pass that ENGINE.md
//! §7.2 risk 6 asks for: the energy quantile ladder is a quantile map, and a
//! quantile map needs a corpus to take its quantiles from. The ladder shipped
//! with constants set from each feature's *definition* rather than fitted, and
//! `constants.rs` says so in its own provenance note.
//!
//! It prints the features and not a verdict, for the same reason
//! [`super::one_track`] does: the fit happens outside, over a corpus split in
//! half, and a harness that scored the fit for you would make it impossible to
//! tell a fitted number from a reported one.
//!
//! Tab-separated rather than comma-separated because the first column is a real
//! filesystem path and real DJ libraries have commas in their filenames.
//!
//! ```text
//! cargo run --release --features eval --bin keystone-eval -- \
//!     --energy-features <file holding one audio path per line>
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::analysis::energy::score::feature_array;
use crate::error::{KeystoneError, Result};
use crate::models::AnalysisResult;

/// Six workers, which is what `ACCURACY.md` measured the full pipeline at.
pub const DEFAULT_THREADS: usize = 6;

pub const HEADER: &str = "path\tduration_ms\tonset_rate\tflux_mean\tlow_ratio\thigh_ratio\t\
                          centroid\tperc_ratio\tdynamic_span\traw\tenergy\tmodel_version";

/// Runs the whole engine over every path in `list` and writes one row per file
/// to stdout, with failures on stderr.
///
/// The whole engine rather than the energy stage alone: energy reads the
/// downbeat grid and the HPSS reductions, so a shortcut here would be measuring
/// something the app never computes.
pub fn run(list: &Path, threads: usize) -> Result<()> {
    let paths = read_list(list)?;
    eprintln!("{} files, {threads} workers", paths.len());

    let rows = analyse_all(&paths, threads);
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{HEADER}");
    let mut failed = 0usize;
    for (path, outcome) in &rows {
        match outcome {
            Ok(result) => {
                let _ = writeln!(out, "{}", row(path, result));
            }
            Err(e) => {
                failed += 1;
                eprintln!("FAILED\t{}\t{e}", path.display());
            }
        }
    }
    eprintln!(
        "{} rows, {failed} failures — failures are listed above and are not rows",
        rows.len() - failed
    );
    Ok(())
}

fn read_list(list: &Path) -> Result<Vec<PathBuf>> {
    let text = std::fs::read_to_string(list)
        .map_err(|e| KeystoneError::NotFound(format!("{}: {e}", list.display())))?;
    let paths: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() {
        return Err(KeystoneError::NotFound(format!(
            "{} holds no paths",
            list.display()
        )));
    }
    Ok(paths)
}

/// In list order, so a re-run diffs against the last one line for line.
fn analyse_all(paths: &[PathBuf], threads: usize) -> Vec<(PathBuf, Result<AnalysisResult>)> {
    let done = std::sync::atomic::AtomicUsize::new(0);
    let total = paths.len();
    let work = || {
        paths
            .par_iter()
            .map(|p| {
                let r = super::one_track::run(p);
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % 50 == 0 {
                    eprintln!("  {n}/{total}");
                }
                (p.clone(), r)
            })
            .collect()
    };
    match rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
    {
        Ok(pool) => pool.install(work),
        Err(_) => paths
            .iter()
            .map(|p| (p.clone(), super::one_track::run(p)))
            .collect(),
    }
}

/// Full `f32` precision, not the one-track report's two decimals: `high_ratio`
/// sits near 0.012 on a real master, and three printed decimals would throw
/// away most of what `ln` reads.
fn row(path: &Path, r: &AnalysisResult) -> String {
    let x = feature_array(&r.energy.features);
    let mut line = format!("{}\t{}", path.display(), r.duration_ms);
    for v in x {
        line.push('\t');
        line.push_str(&v.to_string());
    }
    line.push_str(&format!(
        "\t{}\t{}\t{}",
        r.energy.raw, r.energy.energy, r.energy_model_version
    ));
    line
}
