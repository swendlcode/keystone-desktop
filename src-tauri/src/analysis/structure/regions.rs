//! Sections: consecutive retained boundaries at the 8- or 16-bar level, each
//! given a kind the deferred deck view and the waveform's energy labels read.

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::{boundaries, Boundary, StrongBeatFeatures, StructureFeature};
use crate::error::Result;
use crate::models::{DetectedRegion, RegionKind};

/// A breakdown is percussion dropping out *while the harmony stays*. Both
/// halves matter: percussion alone also describes an intro, and the pair is
/// what separates "the beat stopped" from "the track stopped".
pub const BREAKDOWN_PERCUSSIVE_FRACTION: f32 = 0.4;
pub const BREAKDOWN_HARMONIC_FRACTION: f32 = 0.4;

/// An intro reads as percussion well under the track's **loudest** percussion,
/// not under its median.
///
/// Against the median it inverts on exactly the tracks where it matters: a
/// track that is half quiet drags its own median down until the intro no longer
/// looks quiet, and an ambient track has no loud half to be compared against at
/// all. The same fraction-of-maximum rule is what S11's CUE 1 uses, so the
/// section label and the intro cue cannot disagree.
pub const INTRO_PERCUSSIVE_FRACTION: f32 = 0.25;

/// And an outro reads as percussion under half of it.
pub const OUTRO_PERCUSSIVE_FRACTION: f32 = 0.5;

/// A section shorter than this is a fill, not a section, and gets folded into
/// its predecessor.
pub const MIN_REGION_BARS: i32 = 4;

/// How much of a section's head or tail decides whether it is an intro or an
/// outro, in bars.
///
/// Its head, not its mean: when no boundary was retained between the intro and
/// the build, section 0 covers both, and its *mean* percussion is then high
/// enough to read as a build. How a section starts is also what a DJ reads off
/// the waveform, so it is the better question anyway.
pub const EDGE_BARS: usize = 4;

pub fn build(
    bounds: &[Boundary],
    features: &StrongBeatFeatures,
    grid: &Grid,
    duration_ms: i64,
) -> Result<Vec<DetectedRegion>> {
    let edges = edge_positions(bounds, grid, duration_ms);
    if edges.len() < 2 {
        return Ok(Vec::new());
    }

    let stats = TrackStats::new(features);
    let spans: Vec<(f64, f64)> = edges.windows(2).map(|w| (w[0], w[1])).collect();
    let means: Vec<SpanMeans> = spans
        .iter()
        .map(|(a, b)| SpanMeans::new(features, grid, *a, *b))
        .collect();

    let mut out = Vec::with_capacity(spans.len());
    for (i, (start, end)) in spans.iter().enumerate() {
        let bar_index = grid.strong_beat_to_bar(grid.nearest_strong_beat(*start));
        let bars = if grid.bar_ms > 0.0 {
            ((end - start) / grid.bar_ms).round() as i32
        } else {
            0
        };
        out.push(DetectedRegion {
            start_ms: *start,
            end_ms: *end,
            bar_index: Some(bar_index),
            bars: Some(bars),
            kind: classify(i, spans.len(), &means, &stats),
            // S10 fills this with the identical frozen constants, so a section
            // number is comparable with the track number.
            energy: None,
            key_root: None,
            key_mode: None,
        });
    }
    Ok(merge_short(out, grid))
}

fn edge_positions(bounds: &[Boundary], grid: &Grid, duration_ms: i64) -> Vec<f64> {
    // The first section starts at the head of the file, not at the first
    // detected beat: the intro is a section and the waveform labels it.
    let mut edges: Vec<f64> = vec![0.0];
    for b in boundaries::section_edges(bounds) {
        if b.position_ms > edges[edges.len() - 1] + grid.bar_ms {
            edges.push(b.position_ms);
        }
    }
    let end = duration_ms as f64;
    if end > edges[edges.len() - 1] {
        edges.push(end);
    }
    edges
}

struct TrackStats {
    percussive: f32,
    percussive_max: f32,
    harmonic: f32,
    kick: f32,
}

impl TrackStats {
    fn new(f: &StrongBeatFeatures) -> Self {
        let perc = f.scalar(StructureFeature::PercussiveEnergy);
        Self {
            percussive: median(perc),
            percussive_max: perc.iter().cloned().fold(0.0f32, f32::max),
            harmonic: median(f.scalar(StructureFeature::HarmonicEnergy)),
            kick: median(f.scalar(StructureFeature::KickFlux)),
        }
    }
}

struct SpanMeans {
    percussive: f32,
    /// Mean percussive energy over the first [`EDGE_BARS`] bars.
    head_percussive: f32,
    /// ...and over the last.
    tail_percussive: f32,
    harmonic: f32,
    kick: f32,
    /// True when hat flux and percussive energy both trend upwards across the
    /// span, which is what a build is.
    rising: bool,
}

impl SpanMeans {
    fn new(f: &StrongBeatFeatures, grid: &Grid, start: f64, end: f64) -> Self {
        let lo = grid.nearest_strong_beat(start);
        let hi = grid.nearest_strong_beat(end).max(lo + 1);
        let take = |feature: StructureFeature| -> Vec<f32> {
            let s = f.scalar(feature);
            s.get(lo..hi.min(s.len())).unwrap_or(&[]).to_vec()
        };
        let perc = take(StructureFeature::PercussiveEnergy);
        let hat = take(StructureFeature::HatFlux);
        let edge = grid.bars_to_strong_beats(EDGE_BARS).min(perc.len().max(1));
        Self {
            percussive: mean(&perc),
            head_percussive: mean(perc.get(..edge).unwrap_or(&perc)),
            tail_percussive: mean(perc.get(perc.len().saturating_sub(edge)..).unwrap_or(&perc)),
            harmonic: mean(&take(StructureFeature::HarmonicEnergy)),
            kick: mean(&take(StructureFeature::KickFlux)),
            rising: trends_up(&perc) && trends_up(&hat),
        }
    }
}

fn classify(i: usize, n: usize, means: &[SpanMeans], stats: &TrackStats) -> RegionKind {
    let m = &means[i];

    if i == 0 && m.head_percussive < INTRO_PERCUSSIVE_FRACTION * stats.percussive_max {
        return RegionKind::Intro;
    }
    if i + 1 == n && m.tail_percussive < OUTRO_PERCUSSIVE_FRACTION * stats.percussive_max {
        return RegionKind::Outro;
    }
    if m.percussive < BREAKDOWN_PERCUSSIVE_FRACTION * stats.percussive
        && m.harmonic > BREAKDOWN_HARMONIC_FRACTION * stats.harmonic
    {
        return RegionKind::Breakdown;
    }
    // A build is only a build if something louder follows it; otherwise a long
    // riser into an outro would be labelled as one.
    let followed_by_more = means
        .get(i + 1)
        .map(|next| next.percussive > m.percussive)
        .unwrap_or(false);
    if m.rising && followed_by_more {
        return RegionKind::Build;
    }
    if m.percussive >= stats.percussive && m.kick >= stats.kick {
        return RegionKind::Drop;
    }
    RegionKind::Unknown
}

/// Folds a sub-phrase section into its predecessor. Keeps the predecessor's
/// kind: a 2-bar fill is part of the section it interrupts.
fn merge_short(regions: Vec<DetectedRegion>, grid: &Grid) -> Vec<DetectedRegion> {
    let mut out: Vec<DetectedRegion> = Vec::with_capacity(regions.len());
    for r in regions {
        let too_short = r.bars.unwrap_or(0) < MIN_REGION_BARS;
        match out.last_mut() {
            Some(prev) if too_short => {
                prev.end_ms = r.end_ms;
                if grid.bar_ms > 0.0 {
                    prev.bars = Some(((prev.end_ms - prev.start_ms) / grid.bar_ms).round() as i32);
                }
            }
            _ => out.push(r),
        }
    }
    out
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

pub fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Least-squares slope sign, which beats "last > first" on a noisy riser.
pub fn trends_up(values: &[f32]) -> bool {
    let n = values.len();
    if n < 3 {
        return false;
    }
    let xbar = (n - 1) as f32 / 2.0;
    let ybar = mean(values);
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for (i, y) in values.iter().enumerate() {
        let dx = i as f32 - xbar;
        num += dx * (y - ybar);
        den += dx * dx;
    }
    den > f32::EPSILON && num / den > 0.0
}
